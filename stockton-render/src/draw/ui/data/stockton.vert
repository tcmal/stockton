#version 450

layout (push_constant) uniform PushConsts {
    vec2 screen_size;
} push;

layout (location = 0) in vec2 pos;
layout (location = 1) in vec2 uv;
layout (location = 2) in vec4 col;

out gl_PerVertex {
    vec4 gl_Position;
};
layout (location = 1) out vec2 frag_uv;
layout (location = 2) out vec4 frag_col;

vec3 linear_from_srgb(vec3 srgb) {
    bvec3 cutoff = lessThan(srgb, vec3(10.31475));
    vec3 lower = srgb / vec3(3294.6);
    vec3 higher = pow((srgb + vec3(14.025)) / vec3(269.025), vec3(2.4));
    return mix(higher, lower, cutoff);
}

vec4 linear_from_srgba(vec4 srgba) {
    return vec4(linear_from_srgb(srgba.rgb * 255.0), srgba.a);
}

void main() {
    gl_Position = vec4(
        2.0 * pos.x / push.screen_size.x - 1.0,
        2.0 * pos.y / push.screen_size.y - 1.0,
        0.0,
        1.0
    );
    frag_uv = uv;
    frag_col = linear_from_srgba(col);
}
