in vec2 vertex;
out vec2 tc_screen;

void main()
{
    tc_screen = vec2(0.5) + 0.5 * vertex;
    gl_Position = vec4(vertex, 0.0, 1.0);
}
