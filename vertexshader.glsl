#version 140
in uint vertexdata;
uniform mat4 mvp;

void main() {
	uint bitmask = uint(63); //AND with this to get lowest six bits of data.
	//Extract X:
	float x = float(vertexdata & bitmask);
	//Extract Y:
	float y = float((vertexdata >> 6) & bitmask);
	//Extract Z:
	float z = float((vertexdata >> 12) & bitmask);
    gl_Position = mvp * vec4(x, y, z, 1.0);
}