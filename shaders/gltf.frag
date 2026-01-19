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
    float sum = 0.0;
    
    // 3x3 PCF with hardware comparison (fast!)
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            vec2 offset = vec2(float(x), float(y)) * texel;
            sum += texture(shadowMap, vec4(uv + offset, float(cascadeIndex), depthRef - bias));
        }
    }
    return sum / 9.0;
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

    int cascadeIndex = selectCascade(fragViewDepth);
    float shadow = shadowPCF(cascadeIndex, fragWorldPos, normal, diff);

    if (ubo.debugFlags.x > 0.5) {
        vec3 colors[4] = vec3[4](
            vec3(1.0, 0.2, 0.2),
            vec3(0.2, 1.0, 0.2),
            vec3(0.2, 0.4, 1.0),
            vec3(1.0, 1.0, 0.2)
        );
        vec3 c = colors[cascadeIndex];
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
