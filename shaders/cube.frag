#version 450

layout(location = 0) in vec3 fragColor;
layout(location = 1) in vec3 fragNormal;

layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
    vec4 cameraPos;
    vec4 lightDir;
} ubo;

void main() {
    vec3 normal = normalize(fragNormal);
    vec3 lightDir = normalize(ubo.lightDir.xyz);
    
    // Simple diffuse lighting
    float diff = max(dot(normal, lightDir), 0.0);
    
    // Ambient + diffuse
    vec3 ambient = 0.3 * fragColor;
    vec3 diffuse = 0.7 * diff * fragColor;
    
    vec3 result = ambient + diffuse;
    
    outColor = vec4(result, 1.0);
}
