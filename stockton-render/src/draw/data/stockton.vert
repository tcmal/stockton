#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in vec3 colour;

out gl_PerVertex {
  vec4 gl_Position;
};
layout (location = 1) out vec3 frag_colour;

void main()
{
	gl_Position = vec4(position, 0.0, 1.0);
	frag_colour = colour;
}