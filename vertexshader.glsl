#version 140
    in vec2 position;
    uniform mat4 matrix;
    out vec2 pos;

    void main() {
        gl_Position = matrix * vec4(position, 0.0, 1.0);
        pos.x = gl_Position.x;
        pos.y = gl_Position.y;
    }