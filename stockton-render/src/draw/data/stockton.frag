#version 450

// DescriptorSet 0 = Matrices
// DescriptorSet 1 = Textures
layout(set = 1, binding = 0) uniform texture2D tex[2];
layout(set = 1, binding = 1) uniform sampler samp[2];

layout (location = 1) in vec2 frag_uv;
layout (location = 2) in flat int frag_tex;

layout (location = 0) out vec4 color;

void main()
{
	color = texture(sampler2D(tex[frag_tex], samp[frag_tex]), frag_uv);
}