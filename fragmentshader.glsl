#version 140
	in vec2 v_tex_coords;
    flat in uint v_tex_id;
    out vec4 color;
	
	uniform sampler2DArray tex;
	//uniform sampler2D tex;
	//uniform vec4 color_u = vec4(1.0, 0.0, 0.0, 1.0);

    void main() {
        color = texture(tex, vec3(v_tex_coords, float(v_tex_id))); //color_u;
    }