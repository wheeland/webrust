uniform float invTargetSize;
uniform float radius;
uniform sampler2D positions;

layout(location = 0) out vec3 normal;

vec3 _pos(vec2 pixel) {
    vec4 heightPos = texture(positions, pixel * invTargetSize);
    return heightPos.xyz * (radius + heightPos.w);
}

void main()
{
    //
    // Get coordinates of neighbor vertices and build normal
    //
    vec3 xp = _pos(gl_FragCoord.xy + vec2(1.0,  0.0));
    vec3 xn = _pos(gl_FragCoord.xy + vec2(-1.0, 0.0));
    vec3 yp = _pos(gl_FragCoord.xy + vec2(0.0,  1.0));
    vec3 yn = _pos(gl_FragCoord.xy + vec2(0.0, -1.0));

    vec3 norm = normalize(cross(xp - xn, yp - yn));
    if (dot(norm, xp) < 0.0)
        norm = -norm;

    normal = vec3(0.5) + 0.5 * norm;
}
