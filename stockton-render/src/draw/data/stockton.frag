#version 450

layout(location = 1) in vec3 frag_colour;

layout(location = 0) out vec4 colour;

void main()
{
	colour = vec4(frag_colour, 1.0);
}