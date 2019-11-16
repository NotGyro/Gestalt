#version 450

layout(location = 0) in flat uint id;

layout(location = 0) out uint result;

void main() {
    result = id;
}
