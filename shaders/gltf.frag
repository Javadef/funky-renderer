#version 450

layout(location = 0) in vec3 fragColor;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec2 fragTexCoord;
layout(location = 3) in vec3 fragWorldPos;
layout(location = 4) in float fragViewDepth;

layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
    vec4 cameraPos;
    vec4 lightDir;
    mat4 lightViewProj[4];
    vec4 cascadeSplits;
    vec4 shadowMapSize; // (w,h,1/w,1/h)
    vec4 debugFlags;    // x = debug cascades, y = use PCSS, z = shadow TAA
    vec4 shadowBias;    // x = pcf slope-scale, y = pcf min-bias

    mat4 prevViewProj;
} ubo;

layout(push_constant) uniform PushConstants {
    mat4 model;
    int useTexture;
} pc;

layout(binding = 1) uniform sampler2D texSampler;
layout(binding = 2) uniform sampler2DArrayShadow shadowMap;  // Hardware shadow comparison
layout(binding = 3) uniform sampler2DArray shadowMapDepth;   // Raw depth for PCSS blocker search
layout(binding = 4) uniform sampler2D shadowHistory;          // Previous frame history: (shadow, ndcDepth)
layout(rg16f, binding = 5) uniform image2D shadowHistoryOut;   // Current frame history write: (shadow, ndcDepth)
layout(binding = 6) uniform sampler2D sceneDepthLinear;       // Scene depth with bilinear filtering (for contact shadows)
layout(binding = 7) uniform sampler2D sceneDepthNearest;      // Scene depth with nearest filtering (for contact shadows)

struct ShadowResult {
    float v;
    float m1;
    float m2;
    float kernelRadiusTexels;
};

int selectCascade(float viewDepth) {
    if (viewDepth < ubo.cascadeSplits.x) return 0;
    if (viewDepth < ubo.cascadeSplits.y) return 1;
    if (viewDepth < ubo.cascadeSplits.z) return 2;
    return 3;
}

void selectCascadeBlend(float viewDepth, out int c0, out int c1, out float t) {
    // Blend across cascade boundaries to avoid a hard "switch seam".
    float s0 = ubo.cascadeSplits.x;
    float s1 = ubo.cascadeSplits.y;
    float s2 = ubo.cascadeSplits.z;
    float f0 = max(0.10 * s0, 0.5);
    float f1 = max(0.10 * s1, 0.5);
    float f2 = max(0.10 * s2, 0.5);

    if (viewDepth > s0 - f0 && viewDepth < s0 + f0) {
        c0 = 0; c1 = 1;
        t = smoothstep(s0 - f0, s0 + f0, viewDepth);
        return;
    }
    if (viewDepth > s1 - f1 && viewDepth < s1 + f1) {
        c0 = 1; c1 = 2;
        t = smoothstep(s1 - f1, s1 + f1, viewDepth);
        return;
    }
    if (viewDepth > s2 - f2 && viewDepth < s2 + f2) {
        c0 = 2; c1 = 3;
        t = smoothstep(s2 - f2, s2 + f2, viewDepth);
        return;
    }

    c0 = selectCascade(viewDepth);
    c1 = c0;
    t = 0.0;
}

// Interleaved Gradient Noise - much less visible pattern than sin-hash
// From: http://www.iryoku.com/next-generation-post-processing-in-call-of-duty-advanced-warfare
float interleavedGradientNoise(vec2 screenPos) {
    vec3 magic = vec3(0.06711056, 0.00583715, 52.9829189);
    return fract(magic.z * fract(dot(screenPos, magic.xy)));
}

float shadowFramePhi(vec2 screenPos) {
    // If the sampling pattern is stable (no per-frame change), TAA cannot denoise it.
    // We animate the rotation only when shadow TAA is enabled.
    float frame = ubo.debugFlags.w;
    vec2 p = screenPos;
    if (ubo.debugFlags.z > 0.5) {
        p += vec2(frame * 13.37, frame * 17.17);
    }
    return interleavedGradientNoise(p) * 6.2831853;
}

mat2 rot2(float a) {
    float s = sin(a);
    float c = cos(a);
    return mat2(c, -s, s, c);
}

// Vogel disk - better distribution than Poisson, parametric
vec2 vogelDiskSample(int sampleIndex, int sampleCount, float phi) {
    float goldenAngle = 2.4;
    float r = sqrt(float(sampleIndex) + 0.5) / sqrt(float(sampleCount));
    float theta = float(sampleIndex) * goldenAngle + phi;
    return vec2(r * cos(theta), r * sin(theta));
}

// ============================================================================
// TINY GLADE STYLE CONTACT SHADOWS - Screen-space ray march toward light
// Based on Tomasz Stachowiak's raymarch.hlsl (kajiya/Tiny Glade)
// ============================================================================

// Convert clip-space XY to UV
vec2 csToUv(vec2 cs) {
    return cs * 0.5 + 0.5;
}

// Convert UV to clip-space XY
vec2 uvToCs(vec2 uv) {
    return uv * 2.0 - 1.0;
}

// Get linear depth from NDC depth (Vulkan: 0=near, 1=far with reverse-Z or standard)
// For standard depth: linearDepth = near * far / (far - depth * (far - near))
// We use view-space Z directly for simplicity
float getLinearDepth(float ndcDepth) {
    // Reconstruct linear depth from projection
    // For Vulkan with standard projection: z_linear = proj[3][2] / (ndcDepth - proj[2][2])
    // Simplified for typical perspective: we use the stored view depth
    float near = 0.1;
    float far = 1000.0;
    return near * far / (far - ndcDepth * (far - near));
}

// The magic: sample depth with BOTH linear and nearest filtering
// This is Tiny Glade's key insight for artifact-free ray marching
struct DepthSample {
    float linearDepth;      // Bilinear filtered (smooth surface)
    float unfilteredDepth;  // Point sampled (discrete surface)
    float maxDepth;         // max of both (conservative for hit detection)
    float minDepth;         // min of both (conservative for penetration)
};

DepthSample sampleDepthDual(vec2 uv) {
    DepthSample d;
    
    // Sample with bilinear filtering - reconstructs smooth surface
    float rawLinear = texture(sceneDepthLinear, uv).r;
    // Sample with nearest filtering - discrete "duplo brick" surface  
    float rawNearest = texture(sceneDepthNearest, uv).r;
    
    // Convert to linear depth (reciprocal for proper interpolation)
    d.linearDepth = 1.0 / max(rawLinear, 0.0001);
    d.unfilteredDepth = 1.0 / max(rawNearest, 0.0001);
    
    // The trick: use both to reject false occlusions
    // - Linear fixes "duplo brick" stair-stepping artifacts
    // - Nearest fixes "shrink-wrap" false shadows at edges
    d.maxDepth = max(d.linearDepth, d.unfilteredDepth);
    d.minDepth = min(d.linearDepth, d.unfilteredDepth);
    
    return d;
}

// Contact shadow ray march result
struct ContactShadowResult {
    bool hit;
    float hitT;             // 0..1 along ray
    float penetration;      // How far ray went behind surface
};

// Screen-space ray march toward light for contact shadows
// Uses hybrid root finding: linear steps + bisection refinement
ContactShadowResult rayMarchContactShadow(
    vec3 worldPos,
    vec3 lightDir,
    float maxDistance,      // World-space max trace distance
    int linearSteps,        // Number of linear march steps
    int bisectionSteps,     // Refinement steps after hit
    float depthThickness,   // Max depth behind surface to consider valid
    float jitter            // 0..1 jitter for temporal variation
) {
    ContactShadowResult result;
    result.hit = false;
    result.hitT = 1.0;
    result.penetration = 0.0;
    
    // Ray start and end in world space
    vec3 rayStart = worldPos;
    vec3 rayEnd = worldPos + lightDir * maxDistance;
    
    // Transform to clip space
    vec4 startClip = ubo.proj * ubo.view * vec4(rayStart, 1.0);
    vec4 endClip = ubo.proj * ubo.view * vec4(rayEnd, 1.0);
    
    // Perspective divide
    vec3 startCs = startClip.xyz / startClip.w;
    vec3 endCs = endClip.xyz / endClip.w;
    
    // Clip ray to screen bounds
    vec3 rayDir = endCs - startCs;
    
    // Clip to NDC bounds [-1,1] for XY, [0,1] for Z (Vulkan)
    float tMin = 0.0;
    float tMax = 1.0;
    
    // Clip X
    if (abs(rayDir.x) > 0.0001) {
        float t1 = (-1.0 - startCs.x) / rayDir.x;
        float t2 = (1.0 - startCs.x) / rayDir.x;
        if (rayDir.x < 0.0) { float tmp = t1; t1 = t2; t2 = tmp; }
        tMin = max(tMin, t1);
        tMax = min(tMax, t2);
    }
    // Clip Y
    if (abs(rayDir.y) > 0.0001) {
        float t1 = (-1.0 - startCs.y) / rayDir.y;
        float t2 = (1.0 - startCs.y) / rayDir.y;
        if (rayDir.y < 0.0) { float tmp = t1; t1 = t2; t2 = tmp; }
        tMin = max(tMin, t1);
        tMax = min(tMax, t2);
    }
    // Clip Z (Vulkan: 0..1)
    if (abs(rayDir.z) > 0.0001) {
        float t1 = (0.0 - startCs.z) / rayDir.z;
        float t2 = (1.0 - startCs.z) / rayDir.z;
        if (rayDir.z < 0.0) { float tmp = t1; t1 = t2; t2 = tmp; }
        tMin = max(tMin, t1);
        tMax = min(tMax, t2);
    }
    
    if (tMin >= tMax) {
        return result; // Ray misses screen
    }
    
    // Clipped ray
    vec3 marchStart = startCs + rayDir * tMin;
    vec3 marchEnd = startCs + rayDir * tMax;
    vec3 marchDir = marchEnd - marchStart;
    
    // Linear march phase
    float minT = 0.0;
    float maxT = 1.0;
    bool intersected = false;
    float lastPenetration = 0.0;
    
    for (int step = 0; step < linearSteps; step++) {
        // Jittered step position (for TAA convergence)
        float t = (float(step) + jitter) / float(linearSteps);
        vec3 sampleCs = marchStart + marchDir * t;
        
        // Sample UV
        vec2 sampleUv = csToUv(sampleCs.xy);
        
        // Bounds check
        if (sampleUv.x < 0.0 || sampleUv.x > 1.0 || sampleUv.y < 0.0 || sampleUv.y > 1.0) {
            continue;
        }
        
        // The magic dual-sampler depth read
        DepthSample depth = sampleDepthDual(sampleUv);
        
        // Ray depth in linear space
        float rayLinearDepth = 1.0 / max(sampleCs.z, 0.0001);
        
        // Distance to surface (negative = behind surface)
        float distance = depth.maxDepth - rayLinearDepth;
        
        // Penetration check
        float penetration = rayLinearDepth - depth.minDepth;
        
        // Valid if we haven't gone too far behind the surface
        bool valid = penetration < depthThickness;
        
        if (distance < 0.0 && valid) {
            // Hit! Record interval for bisection
            maxT = t;
            intersected = true;
            lastPenetration = penetration;
            break;
        } else {
            minT = t;
        }
    }
    
    // Bisection refinement
    if (intersected && bisectionSteps > 0) {
        for (int step = 0; step < bisectionSteps; step++) {
            float midT = (minT + maxT) * 0.5;
            vec3 sampleCs = marchStart + marchDir * midT;
            vec2 sampleUv = csToUv(sampleCs.xy);
            
            DepthSample depth = sampleDepthDual(sampleUv);
            float rayLinearDepth = 1.0 / max(sampleCs.z, 0.0001);
            float distance = depth.maxDepth - rayLinearDepth;
            float penetration = rayLinearDepth - depth.minDepth;
            bool valid = penetration < depthThickness;
            
            if (distance < 0.0 && valid) {
                maxT = midT;
                lastPenetration = penetration;
            } else {
                minT = midT;
            }
        }
    }
    
    if (intersected) {
        result.hit = true;
        result.hitT = maxT;
        result.penetration = lastPenetration;
    }
    
    return result;
}

// Compute contact shadow with soft falloff
float computeContactShadow(vec3 worldPos, vec3 normal, vec3 lightDir) {
    // Only trace if surface faces the light (skip backfaces)
    float NdotL = dot(normal, lightDir);
    if (NdotL <= 0.0) {
        return 1.0; // No contact shadow contribution for backfaces
    }
    
    // Offset start position slightly along normal to avoid self-intersection
    vec3 startPos = worldPos + normal * 0.01;
    
    // Trace distance based on scene scale (short range for contact shadows)
    float traceDistance = 0.5; // World units - adjust based on your scene scale
    
    // Jitter for TAA (frame-based)
    float jitter = interleavedGradientNoise(gl_FragCoord.xy + vec2(ubo.debugFlags.w * 13.37, ubo.debugFlags.w * 17.17));
    
    // Ray march parameters (Tiny Glade style: 8 linear + 4 bisection)
    ContactShadowResult cs = rayMarchContactShadow(
        startPos,
        lightDir,
        traceDistance,
        8,              // linear steps
        4,              // bisection steps  
        0.05,           // depth thickness (world units)
        jitter
    );
    
    if (cs.hit) {
        // Soft falloff based on how far along the ray we hit
        // Closer hits = darker shadow
        float shadowStrength = 1.0 - smoothstep(0.0, 0.5, cs.hitT);
        
        // Also fade based on penetration (thin surfaces = softer shadow)
        float penetrationFade = 1.0 - smoothstep(0.0, 0.05, cs.penetration);
        
        return 1.0 - shadowStrength * penetrationFade * 0.8; // 0.8 = max darkness
    }
    
    return 1.0; // No occlusion
}

// PCSS blocker search - finds average blocker depth
float findBlockerDepth(int cascadeIndex, vec2 uv, float receiverDepth, float searchRadius, vec2 texel) {
    float blockerSum = 0.0;
    float blockerCount = 0.0;
    
    // Use interleaved gradient noise for rotation
    float phi = shadowFramePhi(gl_FragCoord.xy);
    
    const int BLOCKER_SAMPLES = 16;
    for (int i = 0; i < BLOCKER_SAMPLES; i++) {
        vec2 offset = vogelDiskSample(i, BLOCKER_SAMPLES, phi) * searchRadius * texel;
        float depth = texture(shadowMapDepth, vec3(uv + offset, float(cascadeIndex))).r;
        
        if (depth < receiverDepth) {
            blockerSum += depth;
            blockerCount += 1.0;
        }
    }
    
    if (blockerCount > 0.0) {
        return blockerSum / blockerCount;
    }
    return -1.0; // No blockers found
}

// PCSS with contact hardening (Tiny Glade style)
ShadowResult shadowPCSS(int cascadeIndex, vec3 worldPos, vec3 normalWs, float NdotL) {
    // Normal offset bias (Tiny Glade emphasizes this)
    float normalBias = 0.02 * (1.0 - NdotL);
    vec3 biasedWorldPos = worldPos + normalWs * normalBias;

    vec4 lightClip = ubo.lightViewProj[cascadeIndex] * vec4(biasedWorldPos, 1.0);
    vec3 proj = lightClip.xyz / lightClip.w;

    vec2 uv = proj.xy * 0.5 + 0.5;
    float receiverDepth = proj.z;

    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return ShadowResult(1.0, 1.0, 1.0, 0.0);
    }

    // Depth bias
    float baseBias = 0.0008;
    float slopeBias = 0.0025 * (1.0 - NdotL);
    float bias = baseBias + slopeBias;
    receiverDepth -= bias;

    vec2 texel = ubo.shadowMapSize.zw;
    
    // Light size in texels - controls overall softness
    float lightSizeTexels = ubo.shadowBias.x * 2.0;
    
    // Step 1: Blocker search
    float blockerDepth = findBlockerDepth(cascadeIndex, uv, receiverDepth, lightSizeTexels, texel);
    
    // No blockers = fully lit
    if (blockerDepth < 0.0) {
        return ShadowResult(1.0, 1.0, 1.0, 0.0);
    }
    
    // Step 2: Penumbra estimation (contact hardening)
    // penumbraWidth = (receiverDepth - blockerDepth) / blockerDepth * lightSize
    float penumbraRatio = (receiverDepth - blockerDepth) / blockerDepth;
    float penumbraWidth = penumbraRatio * lightSizeTexels;
    
    // Clamp penumbra to reasonable range
    penumbraWidth = clamp(penumbraWidth, 0.5, lightSizeTexels * 2.0);
    
    // Step 3: PCF with penumbra-sized kernel
    float phi = shadowFramePhi(gl_FragCoord.xy);
    
    const int PCF_SAMPLES = 16;
    float shadow = 0.0;
    float shadow2 = 0.0;
    
    for (int i = 0; i < PCF_SAMPLES; i++) {
        vec2 offset = vogelDiskSample(i, PCF_SAMPLES, phi) * penumbraWidth * texel;
        float s = texture(shadowMap, vec4(uv + offset, float(cascadeIndex), receiverDepth));
        shadow += s;
        shadow2 += s * s;
    }

    float m1 = shadow / float(PCF_SAMPLES);
    float m2 = shadow2 / float(PCF_SAMPLES);
    return ShadowResult(m1, m1, m2, penumbraWidth);
}

// Standard PCF (fast path, no contact hardening)
ShadowResult shadowPCF(int cascadeIndex, vec3 worldPos, vec3 normalWs, float NdotL) {
    float normalBias = 0.02 * (1.0 - NdotL);
    vec3 biasedWorldPos = worldPos + normalWs * normalBias;

    vec4 lightClip = ubo.lightViewProj[cascadeIndex] * vec4(biasedWorldPos, 1.0);
    vec3 proj = lightClip.xyz / lightClip.w;

    vec2 uv = proj.xy * 0.5 + 0.5;
    float depthRef = proj.z;

    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return ShadowResult(1.0, 1.0, 1.0, 0.0);
    }

    float baseBias = 0.0008;
    float slopeBias = 0.0025 * (1.0 - NdotL);
    float bias = baseBias + slopeBias;

    vec2 texel = ubo.shadowMapSize.zw;
    float radiusTexels = max(ubo.shadowBias.x, 0.5);

    // Cheap 3x3 for hard shadows
    if (radiusTexels <= 1.25) {
        float sum = 0.0;
        float sum2 = 0.0;
        for (int y = -1; y <= 1; y++) {
            for (int x = -1; x <= 1; x++) {
                vec2 offset = vec2(float(x), float(y)) * texel;
                float s = texture(shadowMap, vec4(uv + offset, float(cascadeIndex), depthRef - bias));
                sum += s;
                sum2 += s * s;
            }
        }
        float m1 = sum / 9.0;
        float m2 = sum2 / 9.0;
        return ShadowResult(m1, m1, m2, 1.0);
    }

    // Vogel disk PCF with interleaved gradient noise
    float phi = shadowFramePhi(gl_FragCoord.xy);
    
    const int TAP_COUNT = 16;
    float sum = 0.0;
    float sum2 = 0.0;
    
    for (int i = 0; i < TAP_COUNT; i++) {
        vec2 offset = vogelDiskSample(i, TAP_COUNT, phi) * radiusTexels * texel;
        float s = texture(shadowMap, vec4(uv + offset, float(cascadeIndex), depthRef - bias));
        sum += s;
        sum2 += s * s;
    }
    float m1 = sum / float(TAP_COUNT);
    float m2 = sum2 / float(TAP_COUNT);
    return ShadowResult(m1, m1, m2, radiusTexels);
}

// Main shadow function - switches between PCF and PCSS based on debugFlags.y
ShadowResult computeShadow(int cascadeIndex, vec3 worldPos, vec3 normalWs, float NdotL) {
    bool usePCSS = ubo.debugFlags.y > 0.5;
    
    if (usePCSS) {
        return shadowPCSS(cascadeIndex, worldPos, normalWs, NdotL);
    } else {
        return shadowPCF(cascadeIndex, worldPos, normalWs, NdotL);
    }
}

ShadowResult mixShadowResult(ShadowResult a, ShadowResult b, float t) {
    ShadowResult r;
    r.v = mix(a.v, b.v, t);
    r.m1 = mix(a.m1, b.m1, t);
    r.m2 = mix(a.m2, b.m2, t);
    r.kernelRadiusTexels = mix(a.kernelRadiusTexels, b.kernelRadiusTexels, t);
    return r;
}

float applyShadowTAA(ShadowResult cur, vec3 worldPos) {
    float currentShadow = cur.v;

    // Always write something so history stays valid.
    float outShadow = currentShadow;

    // Store current-frame depth for next frame's disocclusion rejection.
    vec4 curClip = ubo.proj * ubo.view * vec4(worldPos, 1.0);
    float curNdcDepth = (curClip.w != 0.0) ? (curClip.z / curClip.w) : 1.0;
    curNdcDepth = clamp(curNdcDepth, 0.0, 1.0);

    bool enableTaa = ubo.debugFlags.z > 0.5;
    if (enableTaa) {
        ivec2 historySizeI = imageSize(shadowHistoryOut);
        vec2 historySize = vec2(max(historySizeI.x, 1), max(historySizeI.y, 1));
        vec2 currentUv = (gl_FragCoord.xy + vec2(0.5)) / historySize;

        vec4 prevClip = ubo.prevViewProj * vec4(worldPos, 1.0);
        if (prevClip.w > 0.0) {
            vec3 prevNdc = prevClip.xyz / prevClip.w;
            vec2 prevUv = prevNdc.xy * 0.5 + 0.5;
            // Vulkan NDC depth is 0..1
            bool inBounds = (prevUv.x >= 0.0 && prevUv.x <= 1.0 && prevUv.y >= 0.0 && prevUv.y <= 1.0 && prevNdc.z >= 0.0 && prevNdc.z <= 1.0);
            if (inBounds) {
                vec2 history = texture(shadowHistory, prevUv).rg;
                float historyShadow = history.x;
                float historyDepth = history.y;

                // Disocclusion / mismatch rejection:
                // - large motion => history likely unrelated
                // - large shadow delta => reject to avoid "see-through" bleed
                float motion = length(prevUv - currentUv);
                float delta = abs(historyShadow - currentShadow);
                float depthDelta = abs(historyDepth - prevNdc.z);
                if (motion > 0.02 || depthDelta > 0.02 || delta > 0.35) {
                    outShadow = currentShadow;
                    imageStore(shadowHistoryOut, ivec2(gl_FragCoord.xy), vec4(outShadow, curNdcDepth, 0.0, 0.0));
                    return outShadow;
                }

                // Variance clamp around the current neighborhood estimate.
                float variance = max(0.0, cur.m2 - cur.m1 * cur.m1);
                float stdev = sqrt(variance);
                float softness = clamp(cur.kernelRadiusTexels / 8.0, 0.0, 1.0);

                // Softer shadows => tighter clamp (prevents history bleed).
                float sigma = mix(2.5, 0.9, softness);
                float lo = cur.m1 - sigma * stdev;
                float hi = cur.m1 + sigma * stdev;
                float historyClamped = clamp(historyShadow, lo, hi);

                // Softer shadows benefit from more history weight (reduces crawl),
                // but cap it to avoid ghosting.
                float historyWeight = mix(0.55, 0.85, softness);
                outShadow = mix(currentShadow, historyClamped, historyWeight);
            }
        }
    }

    imageStore(shadowHistoryOut, ivec2(gl_FragCoord.xy), vec4(outShadow, curNdcDepth, 0.0, 0.0));
    return outShadow;
}

void main() {
    // Sample texture unless disabled (used for the ground plane)
    vec4 texColor = (pc.useTexture != 0) ? texture(texSampler, fragTexCoord) : vec4(1.0);
    
    vec3 normal = normalize(fragNormal);
    vec3 lightDir = normalize(ubo.lightDir.xyz);
    vec3 viewDir = normalize(ubo.cameraPos.xyz);
    
    // Strong directional light
    float NdotL = dot(normal, lightDir);
    float diff = max(NdotL, 0.0);

    int c0, c1;
    float ct;
    selectCascadeBlend(fragViewDepth, c0, c1, ct);

    ShadowResult s0 = computeShadow(c0, fragWorldPos, normal, diff);
    ShadowResult s = s0;
    if (ct > 0.0) {
        ShadowResult s1 = computeShadow(c1, fragWorldPos, normal, diff);
        s = mixShadowResult(s0, s1, ct);
    }

    float shadow = applyShadowTAA(s, fragWorldPos);
    
    // Apply Tiny Glade style contact shadows (screen-space ray march)
    float contactShadow = computeContactShadow(fragWorldPos, normal, lightDir);
    shadow = min(shadow, contactShadow);

    if (ubo.debugFlags.x > 0.5) {
        vec3 colors[4] = vec3[4](
            vec3(1.0, 0.2, 0.2),
            vec3(0.2, 1.0, 0.2),
            vec3(0.2, 0.4, 1.0),
            vec3(1.0, 1.0, 0.2)
        );
        vec3 c = colors[c0];
        if (ct > 0.0) {
            c = mix(colors[c0], colors[c1], ct);
        }
        outColor = vec4(c * (0.35 + 0.65 * shadow), 1.0);
        return;
    }
    
    // Add a secondary fill light from opposite side
    vec3 fillLightDir = normalize(vec3(-0.5, 0.3, -0.8));
    float fillDiff = max(dot(normal, fillLightDir), 0.0) * 0.3;
    
    // Specular highlight (Blinn-Phong)
    vec3 halfDir = normalize(lightDir + viewDir);
    float spec = pow(max(dot(normal, halfDir), 0.0), 32.0);
    
    // Combine lighting with texture
    vec3 baseColor = texColor.rgb * fragColor;
    vec3 ambient = 0.25 * baseColor;
    vec3 diffuse = 0.65 * diff * baseColor * shadow;
    vec3 fill = fillDiff * baseColor;
    float specFactor = (pc.useTexture != 0) ? 1.0 : 0.0;
    vec3 specular = vec3(0.3) * spec * specFactor;
    
    vec3 result = ambient + diffuse + fill + specular;
    
    outColor = vec4(result, texColor.a);
}
