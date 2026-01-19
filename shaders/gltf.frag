#version 450

layout(location = 0) in vec3 fragColor;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
    vec4 cameraPos;
    vec4 lightDir;
} ubo;

layout(push_constant) uniform PushConstants {
    mat4 model;
    int useTexture;
} pc;

layout(binding = 1) uniform sampler2D texSampler;

void main() {
    // Sample texture unless disabled (used for the ground plane)
    vec4 texColor = (pc.useTexture != 0) ? texture(texSampler, fragTexCoord) : vec4(1.0);
    
    vec3 normal = normalize(fragNormal);
    vec3 lightDir = normalize(ubo.lightDir.xyz);
    vec3 viewDir = normalize(ubo.cameraPos.xyz);
    
    // Strong directional light
    float NdotL = dot(normal, lightDir);
    float diff = max(NdotL, 0.0);
    
    // Add a secondary fill light from opposite side
    vec3 fillLightDir = normalize(vec3(-0.5, 0.3, -0.8));
    float fillDiff = max(dot(normal, fillLightDir), 0.0) * 0.3;
    
    // Specular highlight (Blinn-Phong)
    vec3 halfDir = normalize(lightDir + viewDir);
    float spec = pow(max(dot(normal, halfDir), 0.0), 32.0);
    
    // Combine lighting with texture
    vec3 baseColor = texColor.rgb * fragColor;
    vec3 ambient = 0.25 * baseColor;
    vec3 diffuse = 0.65 * diff * baseColor;
    vec3 fill = fillDiff * baseColor;
    float specFactor = (pc.useTexture != 0) ? 1.0 : 0.0;
    vec3 specular = vec3(0.3) * spec * specFactor;
    
    vec3 result = ambient + diffuse + fill + specular;
    
    outColor = vec4(result, texColor.a);
}
