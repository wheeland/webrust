uniform float vertexGridSize;
uniform float textureDelta;
uniform float textureSize;
uniform float radius;
uniform sampler2D positions;
uniform sampler2D parentCoords;

layout(location = 0) out vec4 posHeight;
layout(location = 1) out vec4 detail;

vec4 getHeightPos(vec2 pixelCenter) {
    vec2 texPixel = (pixelCenter - vec2(0.5)) * textureDelta + vec2(0.5);
    return texture(positions, texPixel / textureSize);
}

vec3 _pos(vec2 tc) {
    vec4 heightPos = getHeightPos(tc);
    return heightPos.xyz * (radius + heightPos.w);
}

void main()
{
    //
    // Get coordinates of neighbor vertices
    //
    vec4 heightPosCenter = getHeightPos(gl_FragCoord.xy);
    vec3 pCenter = heightPosCenter.xyz * (radius + heightPosCenter.w);

    // get position of parent vertices within this tile (range: [0..1])
    vec4 parents = texture(parentCoords, (gl_FragCoord.xy - vec2(1.0)) / (vertexGridSize + 1.0));

    //
    // calculate interpolated position
    //
    float interpolation = 0.0;
    if (parents.xy != parents.zw) {
        // read parent world positions
        vec3 pparent1 = _pos(vec2(1.5) + parents.xy * vertexGridSize);
        vec3 pparent2 = _pos(vec2(1.5) + parents.zw * vertexGridSize);
        vec3 mid = mix(pparent1, pparent2, 0.5);

        // calculate relative difference to this position
        float dParents = length(pparent1 - pparent2);
        float dMid = length(mid - pCenter);
        interpolation = 0.5 * dMid / dParents * sqrt(length(parents.xy - parents.zw));
    }

    posHeight = heightPosCenter;
    detail = vec4(5.0 * interpolation * sqrt(vertexGridSize));
}
