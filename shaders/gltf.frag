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
    vec4 debugFlags;    // x = debug cascades
    vec4 shadowBias;    // x = pcf slope-scale, y = pcf min-bias
} ubo;

layout(push_constant) uniform PushConstants {
    mat4 model;
    int useTexture;
} pc;

layout(binding = 1) uniform sampler2D texSampler;
layout(binding = 2) uniform sampler2DArrayShadow shadowMap;  // Hardware shadow comparison

int selectCascade(float viewDepth) {
    if (viewDepth < ubo.cascadeSplits.x) return 0;
    if (viewDepth < ubo.cascadeSplits.y) return 1;
    if (viewDepth < ubo.cascadeSplits.z) return 2;
    return 3;
}

void selectCascadeBlend(float viewDepth, out int c0, out int c1, out float t) {
    // Blend across cascade boundaries to avoid a hard "switch seam".
    // Fade width scales with depth so it remains visible but cheap.
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

float hash12(vec2 p) {
    // Cheap stable hash for per-pixel rotation
    float h = dot(p, vec2(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

mat2 rot2(float a) {
    float s = sin(a);
    float c = cos(a);
    return mat2(c, -s, s, c);
}

float shadowPCF(int cascadeIndex, vec3 worldPos, vec3 normalWs, float NdotL) {
    // Receiver-side biasing:
    // - Normal offset reduces "triangle pattern" acne on curved/low-poly surfaces
    // - Small slope-scaled depth bias handles grazing angles
    // These are intentionally tiny because we're using a D32 shadow map.
    float normalBias = 0.02 * (1.0 - NdotL);
    vec3 biasedWorldPos = worldPos + normalWs * normalBias;

    vec4 lightClip = ubo.lightViewProj[cascadeIndex] * vec4(biasedWorldPos, 1.0);
    vec3 proj = lightClip.xyz / lightClip.w;

    vec2 uv = proj.xy * 0.5 + 0.5;
    float depthRef = proj.z;

    // Outside the shadow map => lit.
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) return 1.0;

    // Depth bias in shadow-map depth units.
    // (If acne persists, increase baseBias slightly before touching slopeBias.)
    float baseBias = 0.0010;
    float slopeBias = 0.0030 * (1.0 - NdotL);
    float bias = baseBias + slopeBias;

    vec2 texel = ubo.shadowMapSize.zw;

    // Softness radius in texels (passed from CPU). 1.0 ~= near-hard edge.
    float radiusTexels = max(ubo.shadowBias.x, 0.5);

    float sum = 0.0;

    // Keep cheap 3x3 for near-hard shadows.
    if (radiusTexels <= 1.25) {
        for (int y = -1; y <= 1; y++) {
            for (int x = -1; x <= 1; x++) {
                vec2 offset = vec2(float(x), float(y)) * texel;
                sum += texture(shadowMap, vec4(uv + offset, float(cascadeIndex), depthRef - bias));
            }
        }
        return sum / 9.0;
    }

    // Poisson-disk PCF for softer penumbra.
    const int TAP_COUNT = 12;
    const vec2 poisson[TAP_COUNT] = vec2[TAP_COUNT](
        vec2(-0.326, -0.406), vec2(-0.840, -0.074), vec2(-0.696,  0.457), vec2(-0.203,  0.621),
        vec2( 0.962, -0.195), vec2( 0.473, -0.480), vec2( 0.519,  0.767), vec2( 0.185, -0.893),
        vec2( 0.507,  0.064), vec2( 0.896,  0.412), vec2(-0.322, -0.933), vec2(-0.792, -0.598)
    );

    // Use a screen-space seed so the kernel rotation doesn't change across cascade boundaries.
    float angle = hash12(gl_FragCoord.xy) * 6.2831853;
    mat2 R = rot2(angle);
    vec2 radiusUv = texel * radiusTexels;

    for (int i = 0; i < TAP_COUNT; i++) {
        vec2 offset = (R * poisson[i]) * radiusUv;
        sum += texture(shadowMap, vec4(uv + offset, float(cascadeIndex), depthRef - bias));
    }
    return sum / float(TAP_COUNT);
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

    float shadow0 = shadowPCF(c0, fragWorldPos, normal, diff);
    float shadow = shadow0;
    if (ct > 0.0) {
        float shadow1 = shadowPCF(c1, fragWorldPos, normal, diff);
        shadow = mix(shadow0, shadow1, ct);
    }

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
