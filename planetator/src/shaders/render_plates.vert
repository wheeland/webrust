uniform mat4 mvp;
uniform float radius;
uniform float wf;
uniform float farPlane;
in vec4 posHeight;
in vec2 plateCoords;
out vec2 plateTc;
out vec3 pos;

void main()
{
    plateTc = plateCoords;
    pos = posHeight.xyz * (posHeight.w + radius + 0.0001 * wf);

    vec4 cpos = mvp * vec4(pos, 1.0);
    // float C = 1.0;
    // cpos.z = (2.0 * log(C * wpos.w + 1.0) / log(C * farPlane + 1.0) - 1.0) * wpos.w;

    gl_Position = cpos;
}
