layout(location = 0) out vec4 outNormalWf;
layout(location = 1) out vec4 outPositionHeight;
$CHANNEL_OUTPUTS

uniform sampler2D tex_normals;
uniform sampler2D tex_heights;
$CHANNEL_TEXTURES

uniform float wf;
uniform float radius;

in vec2 plateTc;
in vec3 pos;

void main()
{
    vec3 normalFromTex = texture(tex_normals, plateTc).xyz;
    float height = texture(tex_heights, plateTc).r;
    outNormalWf = vec4(normalFromTex, wf);
    outPositionHeight = vec4(normalize(pos) * (radius + height), height);
    $CHANNEL_ASSIGNMENTS
}
