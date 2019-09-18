layout(location = 0) out vec4 outColorReflectivity;

uniform sampler2D scene_normal;
uniform sampler2D scene_position;
uniform float waterHeight;

in vec2 tc_screen;

vec2 uv1;
vec4 uv23;
vec3 uvDists;

vec3 sceneNormal;
vec3 scenePosition;

$CHANNEL_TEXTURES
$CHANNEL_VARIABLES

$NOISE

$ICOSAHEDRON
$TEXTURE_FUNCTIONS

vec3 color(vec3 normal, vec3 position, float height);

vec3 _normUnit(vec3 v) {
    vec3 L = abs(v);
    return v / (L.x + L.y + L.z);
}

float _maxElem(vec3 v) {
    return max(v.x, max(v.y, v.z));
}

vec2 _projectIntoUvSpace(vec3 position, vec3 normal) {
    float x2 = normal.y + normal.z;
    float y2 = normal.x + normal.z;
    float z2 = (-normal.x * x2 - normal.y * y2) / normal.z;

    vec3 dir1 = normalize(vec3(x2, y2, z2));
    vec3 dir2 = cross(dir1, normal);

    // project onto plane
    vec3 onPlane = position - normal * dot(position, normal);

    float u = dot(onPlane, dir1);
    float v = dot(onPlane, dir2);
    return vec2(u, v);
}

void _generateUvMaps(vec3 n, vec3 position)
{
    float d1 = 0.0;
    float d2 = 0.0;
    float dp = 0.0;
    int i1 = 0;
    int i2 = 0;
    int ip = 0;

    //
    // find highest and second-highest scoring hexagon
    //
    for (int i = 0; i < 20; ++i) {
        float d = dot(n, icoNorms[i]);
        if (d > d1) {
            d2 = d1;
            i2 = i1;
            d1 = d;
            i1 = i;
        } else if (d > d2) {
            d2 = d;
            i2 = i;
        }
    }

    //
    // find highest-scoring pentagon
    //
    for (int i = 0; i < 12; ++i) {
        float d = dot(n, icoVerts[i]);
        if (d > dp) {
            dp = d;
            ip = i;
        }
    }

    // normals of this triangle/hexagon, neighbor triangle/hexagon, and pentagon
    vec3 thisHexNorm = icoNorms[i1];
    vec3 neighborHexNorm = icoNorms[i2];
    vec3 pentNorm = icoVerts[ip];

    // barycentric coordinates of N in this and the neighbor hexagon/triangle UVW space
    vec3 thisUvw = _normUnit(mat3(icoMats1[i1], icoMats2[i1], icoMats3[i1]) * n);
    vec3 neighborUvw = _normUnit(mat3(icoMats1[i2], icoMats2[i2], icoMats3[i2]) * n);

    // relative distance to the neighbor hexagon border, 0 is the border, 1 is this hexagon's center
    float neighborDist = 1.4142 * 3.0 * _maxElem(-neighborUvw);

    // UV spaces for the three adjacent surfaces
    vec2 thisUv = _projectIntoUvSpace(position, thisHexNorm);
    vec2 neighborUv = _projectIntoUvSpace(position, neighborHexNorm);
    vec2 pentUv = _projectIntoUvSpace(position, pentNorm);

    if (all(lessThan(thisUvw, vec3(2.0 / 3.0)))) {
        // relative distance to the pentagon border, 0 is in the border, 1 is this hexagon's center
        float pentDist = 2.0 - 3.0 * _maxElem(thisUvw);

        float fNeighbor = clamp(0.0, neighborDist, 1.0);
        float fPentagon = clamp(0.0, pentDist, 1.0);

        uv1 = thisUv;
        uv23 = vec4(neighborUv, pentUv);
        uvDists = vec3(fNeighbor, fPentagon, 0.0);
    }
    else {
        // relative distance to the pentagon border, 0 is in the border, 1 is this hexagon's center
        float mainDist = 3.0 * _maxElem(thisUvw) - 2.0;

        float fNeighbor = clamp(0.0, neighborDist, 1.0);
        float fMain = clamp(0.0, mainDist, 1.0);

        uv1 = pentUv;
        uv23 = vec4(thisUv, neighborUv);
        uvDists = vec3(fMain, fNeighbor, 1.0);
    }
}

void main()
{
    vec3 normalFromTex = texture(scene_normal, tc_screen).xyz;

    // open sky is encoded as (0,0,0) normal
    if (normalFromTex == vec3(0.0)) {
        outColorReflectivity = vec4(0.0);
        return;
    }

    vec4 scenePosTex = texture(scene_position, tc_screen);

    sceneNormal = vec3(-1.0) + 2.0 * normalFromTex;
    scenePosition = scenePosTex.xyz;

    // water can have any color, so long as it's black.
    if (scenePosTex.w <= waterHeight) {
        outColorReflectivity = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    _generateUvMaps(sceneNormal, scenePosition);

    $CHANNEL_ASSIGNMENTS
    vec3 col = color(sceneNormal, scenePosition, scenePosTex.w);
    outColorReflectivity = vec4(col, 0.0);
}

#line 1
$COLORATOR
