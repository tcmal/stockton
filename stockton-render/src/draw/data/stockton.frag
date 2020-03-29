#version 450

layout(set = 0, binding = 0) uniform texture2D tex[2];
layout(set = 0, binding = 1) uniform sampler samp[2];

layout (location = 1) in vec3 frag_color;
layout (location = 2) in vec2 frag_uv;
layout (location = 3) in flat int frag_tex;

layout (location = 0) out vec4 color;

void main()
{
	if(frag_tex == -1) {
		color = vec4(frag_color, 1.0);
	} else {
		color = texture(sampler2D(tex[frag_tex], samp[frag_tex]), frag_uv);
	}
}