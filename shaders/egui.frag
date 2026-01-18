#version 450

layout(binding = 0) uniform sampler2D u_tex;

layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;

layout(location = 0) out vec4 out_color;

void main() {
    // Premultiplied-alpha output.
    // Our pipeline blend state is:
    //   src = ONE, dst = ONE_MINUS_SRC_ALPHA
    // which expects RGB already multiplied by alpha.
    vec4 tex = texture(u_tex, v_uv);

    // egui vertex colors are premultiplied; the font atlas uses A as coverage.
    // Multiply BOTH RGB and A by coverage so fully-transparent texels contribute 0.
    float a = v_color.a * tex.a;
    out_color = vec4(v_color.rgb * tex.rgb * tex.a, a);
}
