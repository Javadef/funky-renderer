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
    
    // Bright ambient + diffuse
    vec3 ambient = 0.5 * fragColor;
    vec3 diffuse = 0.6 * diff * fragColor;
    
    // Add slight highlight
    float highlight = pow(max(diff, 0.0), 2.0) * 0.2;
    
    vec3 result = ambient + diffuse + vec3(highlight);
    
    outColor = vec4(result, 1.0);
}
