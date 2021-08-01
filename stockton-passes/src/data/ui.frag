#version 450
#extension GL_ARB_separate_shader_objects : enable

// DescriptorSet 0 = Textures
layout(set = 0, binding = 0) uniform texture2D tex[8];
layout(set = 0, binding = 1) uniform sampler samp[8];

layout (location = 1) in vec2 frag_uv;
layout (location = 2) in vec4 frag_col;

layout (location = 0) out vec4 color;

void main() {
    color = texture(sampler2D(tex[0], samp[0]), frag_uv) * frag_col;
}