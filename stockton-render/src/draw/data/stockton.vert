#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in vec3 colour;
layout (location = 2) in vec2 uv;
layout (location = 3) in int tex;

out gl_PerVertex {
	vec4 gl_Position;
};
layout (location = 1) out vec3 frag_colour;
layout (location = 2) out vec2 frag_uv;
layout (location = 3) out flat int frag_tex;

void main()
{
	gl_Position = vec4(position, 0.0, 1.0);
	frag_colour = colour;
	frag_uv = uv;
	frag_tex = tex;
}