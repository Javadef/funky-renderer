#version 450

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 fragColor;
layout(location = 1) out vec3 fragNormal;

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
    vec4 cameraPos;
    vec4 lightDir;
} ubo;

// Calculate face normal from position (for cube)
vec3 calculateNormal(vec3 pos) {
    vec3 absPos = abs(pos);
    if (absPos.x > absPos.y && absPos.x > absPos.z) {
        return vec3(sign(pos.x), 0.0, 0.0);
    } else if (absPos.y > absPos.z) {
        return vec3(0.0, sign(pos.y), 0.0);
    } else {
        return vec3(0.0, 0.0, sign(pos.z));
    }
}

void main() {
    vec4 worldPos = ubo.model * vec4(inPosition, 1.0);
    gl_Position = ubo.proj * ubo.view * worldPos;
    
    // Transform normal to world space
    mat3 normalMatrix = mat3(ubo.model);
    fragNormal = normalize(normalMatrix * calculateNormal(inPosition));
    
    fragColor = inColor;
}
