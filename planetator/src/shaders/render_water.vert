uniform mat4 mvp;
uniform float radius;
uniform float waterHeight;
uniform float farPlane;
in vec4 posHeight;
in float isRibbon;
in vec2 texCoords;
out vec3 pos;
out vec2 tc;

void main()
{
    tc = texCoords;
    pos = posHeight.xyz * (radius + waterHeight - isRibbon);

    vec4 cpos = mvp * vec4(pos, 1.0);
    // float C = 1.0;
    // cpos.z = (2.0 * log(C * wpos.w + 1.0) / log(C * farPlane + 1.0) - 1.0) * wpos.w;

    gl_Position = cpos;
}
