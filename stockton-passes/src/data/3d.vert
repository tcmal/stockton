#version 450

// DescriptorSet 0 = Matrices
layout (push_constant) uniform PushConsts {
    mat4 vp;
} push;

layout (location = 0) in vec3 position;
layout (location = 1) in int tex;
layout (location = 2) in vec2 uv;

out gl_PerVertex {
	vec4 gl_Position;
};
layout (location = 1) out vec2 frag_uv;
layout (location = 2) out flat int frag_tex;

void main()
{
	gl_Position = push.vp * vec4(position, 1.0);
	frag_uv = uv;
	frag_tex = tex;
}