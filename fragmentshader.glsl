#version 140

    in vec2 pos;
    out vec4 color;

    void main() {
        color = vec4((pos.x+1.0) / 2.0, (-pos.x+1.0) / 2.0, ((pos.y-pos.x)+1.0) / 2.0, 1.0);
		//color = vec4(1.0, 1.0, 1.0, 1.0);
    }