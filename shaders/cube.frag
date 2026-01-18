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
    vec3 viewDir = normalize(ubo.cameraPos.xyz);
    
    // Strong directional light to show cube faces clearly
    float NdotL = dot(normal, lightDir);
    float diff = max(NdotL, 0.0);
    
    // Add a secondary fill light from opposite side
    vec3 fillLightDir = normalize(vec3(-0.5, 0.3, -0.8));
    float fillDiff = max(dot(normal, fillLightDir), 0.0) * 0.3;
    
    // Specular highlight (Blinn-Phong)
    vec3 halfDir = normalize(lightDir + viewDir);
    float spec = pow(max(dot(normal, halfDir), 0.0), 64.0);
    
    // Rim/edge lighting to outline the cube
    float rim = 1.0 - max(dot(viewDir, normal), 0.0);
    rim = pow(rim, 2.0) * 0.5;
    
    // Combine lighting - lower ambient so faces are more distinct
    vec3 ambient = 0.20 * fragColor;
    vec3 diffuse = 0.65 * diff * fragColor;
    vec3 fill = fillDiff * fragColor;
    vec3 specular = vec3(0.4) * spec;
    vec3 rimLight = vec3(0.8, 0.9, 1.0) * rim; // Slight blue tint on edges
    
    vec3 result = ambient + diffuse + fill + specular + rimLight;
    
    outColor = vec4(result, 1.0);
}
