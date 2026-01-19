#version 450

layout(location = 0) in vec3 inPosition;

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

layout(push_constant) uniform ShadowPushConstants {
    mat4 model;
    int cascadeIndex;
} pc;

void main() {
    vec4 worldPos = pc.model * vec4(inPosition, 1.0);
    gl_Position = ubo.lightViewProj[pc.cascadeIndex] * worldPos;
}
