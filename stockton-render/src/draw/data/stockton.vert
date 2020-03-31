#version 450

// DescriptorSet 0 = Matrices
layout (binding = 0) uniform UniformBufferObject {
    mat4 vp;
} matrices;

// DescriptorSet 1 = Textures/Samplers

layout (location = 0) in vec3 position;
layout (location = 1) in vec2 uv;
layout (location = 2) in int tex;

out gl_PerVertex {
	vec4 gl_Position;
};
layout (location = 1) out vec2 frag_uv;
layout (location = 2) out flat int frag_tex;

void main()
{
	gl_Position = matrices.vp * vec4(position, 1.0);
	frag_uv = uv;
	frag_tex = tex;
}