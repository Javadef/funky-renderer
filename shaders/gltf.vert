#version 450

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;
layout(location = 2) in vec3 inNormal;
layout(location = 3) in vec2 inTexCoord;

layout(location = 0) out vec3 fragColor;
layout(location = 1) out vec3 fragNormal;
layout(location = 2) out vec2 fragTexCoord;
layout(location = 3) out vec3 fragWorldPos;
layout(location = 4) out float fragViewDepth;

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

void main() {
    vec4 worldPos = pc.model * vec4(inPosition, 1.0);
    gl_Position = ubo.proj * ubo.view * worldPos;

    vec4 viewPos = ubo.view * worldPos;
    fragViewDepth = -viewPos.z; // view-space distance (positive in front)
    fragWorldPos = worldPos.xyz;
    
    // Transform normal to world space (assumes uniform scale)
    mat3 normalMatrix = mat3(pc.model);
    fragNormal = normalize(normalMatrix * inNormal);
    
    fragColor = inColor;
    fragTexCoord = inTexCoord;
}
