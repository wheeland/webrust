uniform vec2 ofs;
uniform float invTargetSize;
uniform float targetBorderOffset;
uniform float stretch;
uniform float stretchAsin;
uniform float mul;
uniform float radius;
uniform int depth;
uniform mat3 cubeTransformMatrix;

layout(location = 0) out vec4 posHeight;
layout(location = 1) out float height;

$CHANNELS
$NOISE

#line 1
$GENERATOR

void main()
{
    vec2 rel = (gl_FragCoord.xy - vec2(targetBorderOffset)) * invTargetSize;
    vec2 rawXy = asin(stretch * (ofs + vec2(mul) * rel)) / stretchAsin;

    vec2 xy = clamp(vec2(-1.0), rawXy, vec2(1.0));
    vec2 diff = abs(rawXy - xy);
    float dz = max(diff.x, diff.y);

    vec3 cubePos = cubeTransformMatrix * vec3(xy, 1.0 - dz);
    vec3 position = normalize(cubePos);

    height = 0.0;
    generate(position * radius, depth);

    posHeight = vec4(position, height);
}
