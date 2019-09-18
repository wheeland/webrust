in vec2 vertex;
out vec2 clipPos;

void main()
{
    clipPos = vertex;
    gl_Position = vec4(vertex, 0.0, 1.0);
}
