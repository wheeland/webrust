void generate(vec3 position, int depth)
{
    vec3 np = position + 0.8 * noise(0.3 * position, 2, 0.5);
    vec2 w = worley3(np, int(radius / 10.0), 0.4);
    height = w.x;
    height *= 0.5 + 0.5 * noise(0.2 * position, 12, 0.5);
    height *= 3.0;
}
