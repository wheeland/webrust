layout(location = 0) out vec4 outNormalWf;
layout(location = 1) out vec4 outPositionHeight;
$CHANNEL_OUTPUTS

uniform sampler2D heights;
uniform sampler2D normals;
$CHANNEL_TEXTURES

in vec3 pos;
in vec2 tc;

void main()
{
    float terrainHeight = texture(heights, tc).r;
    outNormalWf = vec4(texture(normals, tc).xyz, 0.0);
    outPositionHeight = vec4(pos, terrainHeight);
    $CHANNEL_ASSIGNMENTS
}
