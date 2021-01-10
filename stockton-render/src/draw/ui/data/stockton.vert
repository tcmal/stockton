#version 450

layout (push_constant) uniform PushConsts {
    vec2 screen_size;
} push;

layout(location = 0) in vec2 pos;
layout (location = 1) in vec2 uv;
layout (location = 2) in int col; // rgba of u8s

out gl_PerVertex {
    vec4 gl_Position;
};
layout (location = 1) out vec2 frag_uv;
layout (location = 2) out int frag_col;

void main() {
    gl_Position = vec4(
        ((pos.x / push.screen_size.x) * 2.0) - 1.0,
        ((pos.y / push.screen_size.y) * 2.0) - 1.0,
        0.0,
        1.0
    );
    frag_uv = uv;
    frag_col = col;
}
