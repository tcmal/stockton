#version 450

// DescriptorSet 0 = Textures
layout(set = 0, binding = 0) uniform texture2D tex[8];
layout(set = 0, binding = 1) uniform sampler samp[8];

layout (location = 1) in vec2 frag_uv;
layout (location = 2) in flat int frag_tex;

layout (location = 0) out vec4 color;

void main()
{
	color = texture(sampler2D(tex[frag_tex % 8], samp[frag_tex % 8]), frag_uv);
}