pub struct ShaderNoise {
}

impl ShaderNoise {
    pub fn definitions() -> String {
        String::from(
"float _noise_mod289(float x) {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}
vec2 _noise_mod289(vec2 x) {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}
vec3 _noise_mod289(vec3 x) {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}
vec4 _noise_mod289(vec4 x) {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

float _noise_permute(float x) {
    return _noise_mod289(((x*34.0)+1.0)*x);
}
vec3 _noise_permute(vec3 x) {
    return _noise_mod289(((x*34.0)+1.0)*x);
}
vec4 _noise_permute(vec4 x) {
    return _noise_mod289(((x*34.0)+1.0)*x);
}

float _noise_taylorInvSqrt(float r) {
    return 1.79284291400159 - 0.85373472095314 * r;
}
vec4 _noise_taylorInvSqrt(vec4 r) {
    return 1.79284291400159 - 0.85373472095314 * r;
}

float noise(vec2 v) {
    const vec4 C = vec4(0.211324865405187,  // (3.0-sqrt(3.0))/6.0
                        0.366025403784439,  // 0.5*(sqrt(3.0)-1.0)
                       -0.577350269189626,  // -1.0 + 2.0 * C.x
                        0.024390243902439); // 1.0 / 41.0
    // First corner
    vec2 i  = floor(v + dot(v, C.yy) );
    vec2 x0 = v -   i + dot(i, C.xx);

    // Other corners
    vec2 i1;
    //i1.x = step( x0.y, x0.x ); // x0.x > x0.y ? 1.0 : 0.0
    //i1.y = 1.0 - i1.x;
    i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    // x0 = x0 - 0.0 + 0.0 * C.xx ;
    // x1 = x0 - i1 + 1.0 * C.xx ;
    // x2 = x0 - 1.0 + 2.0 * C.xx ;
    vec4 x12 = x0.xyxy + C.xxzz;
    x12.xy -= i1;

    // Permutations
    i = _noise_mod289(i); // Avoid truncation effects in permutation
    vec3 p = _noise_permute( _noise_permute( i.y + vec3(0.0, i1.y, 1.0 ))
          + i.x + vec3(0.0, i1.x, 1.0 ));

    vec3 m = max(0.5 - vec3(dot(x0,x0), dot(x12.xy,x12.xy), dot(x12.zw,x12.zw)), 0.0);
    m = m*m ;
    m = m*m ;

    // Gradients: 41 points uniformly over a line, mapped onto a diamond.
    // The ring size 17*17 = 289 is close to a multiple of 41 (41*7 = 287)

    vec3 x = 2.0 * fract(p * C.www) - 1.0;
    vec3 h = abs(x) - 0.5;
    vec3 ox = floor(x + 0.5);
    vec3 a0 = x - ox;

    // Normalise gradients implicitly by scaling m
    // Approximation of: m *= inversesqrt( a0*a0 + h*h );
    m *= 1.79284291400159 - 0.85373472095314 * ( a0*a0 + h*h );

    // Compute final noise value at P
    vec3 g;
    g.x  = a0.x  * x0.x  + h.x  * x0.y;
    g.yz = a0.yz * x12.xz + h.yz * x12.yw;
    return 130.0 * dot(m, g);
}

float noise(vec3 v) {
    const vec2  C = vec2(1.0/6.0, 1.0/3.0) ;
    const vec4  D = vec4(0.0, 0.5, 1.0, 2.0);

    // First corner
    vec3 i  = floor(v + dot(v, C.yyy) );
    vec3 x0 =   v - i + dot(i, C.xxx) ;

    // Other corners
    vec3 g = step(x0.yzx, x0.xyz);
    vec3 l = 1.0 - g;
    vec3 i1 = min( g.xyz, l.zxy );
    vec3 i2 = max( g.xyz, l.zxy );

    //   x0 = x0 - 0.0 + 0.0 * C.xxx;
    //   x1 = x0 - i1  + 1.0 * C.xxx;
    //   x2 = x0 - i2  + 2.0 * C.xxx;
    //   x3 = x0 - 1.0 + 3.0 * C.xxx;
    vec3 x1 = x0 - i1 + C.xxx;
    vec3 x2 = x0 - i2 + C.yyy; // 2.0*C.x = 1/3 = C.y
    vec3 x3 = x0 - D.yyy;      // -1.0+3.0*C.x = -0.5 = -D.y

    // Permutations
    i = _noise_mod289(i);
    vec4 p = _noise_permute( _noise_permute( _noise_permute(
               i.z + vec4(0.0, i1.z, i2.z, 1.0 ))
             + i.y + vec4(0.0, i1.y, i2.y, 1.0 ))
             + i.x + vec4(0.0, i1.x, i2.x, 1.0 ));

    // Gradients: 7x7 points over a square, mapped onto an octahedron.
    // The ring size 17*17 = 289 is close to a multiple of 49 (49*6 = 294)
    float n_ = 0.142857142857; // 1.0/7.0
    vec3  ns = n_ * D.wyz - D.xzx;

    vec4 j = p - 49.0 * floor(p * ns.z * ns.z);  //  mod(p,7*7)

    vec4 x_ = floor(j * ns.z);
    vec4 y_ = floor(j - 7.0 * x_ );    // mod(j,N)

    vec4 x = x_ *ns.x + ns.yyyy;
    vec4 y = y_ *ns.x + ns.yyyy;
    vec4 h = 1.0 - abs(x) - abs(y);

    vec4 b0 = vec4( x.xy, y.xy );
    vec4 b1 = vec4( x.zw, y.zw );

    //vec4 s0 = vec4(lessThan(b0,0.0))*2.0 - 1.0;
    //vec4 s1 = vec4(lessThan(b1,0.0))*2.0 - 1.0;
    vec4 s0 = floor(b0)*2.0 + 1.0;
    vec4 s1 = floor(b1)*2.0 + 1.0;
    vec4 sh = -step(h, vec4(0.0));

    vec4 a0 = b0.xzyw + s0.xzyw*sh.xxyy ;
    vec4 a1 = b1.xzyw + s1.xzyw*sh.zzww ;

    vec3 p0 = vec3(a0.xy,h.x);
    vec3 p1 = vec3(a0.zw,h.y);
    vec3 p2 = vec3(a1.xy,h.z);
    vec3 p3 = vec3(a1.zw,h.w);

    //Normalise gradients
    vec4 norm = _noise_taylorInvSqrt(vec4(dot(p0,p0), dot(p1,p1), dot(p2, p2), dot(p3,p3)));
    p0 *= norm.x;
    p1 *= norm.y;
    p2 *= norm.z;
    p3 *= norm.w;

    // Mix final noise value
    vec4 m = max(0.6 - vec4(dot(x0,x0), dot(x1,x1), dot(x2,x2), dot(x3,x3)), 0.0);
    m = m * m;
    return 42.0 * dot( m*m, vec4( dot(p0,x0), dot(p1,x1),
                                  dot(p2,x2), dot(p3,x3) ) );
    }

vec4 _noise_grad4(float j, vec4 ip) {
    const vec4 ones = vec4(1.0, 1.0, 1.0, -1.0);
    vec4 p,s;

    p.xyz = floor( fract (vec3(j) * ip.xyz) * 7.0) * ip.z - 1.0;
    p.w = 1.5 - dot(abs(p.xyz), ones.xyz);
    s = vec4(lessThan(p, vec4(0.0)));
    p.xyz = p.xyz + (s.xyz*2.0 - 1.0) * s.www;

    return p;
}

float noise(vec4 v) {
    const vec4  C = vec4( 0.138196601125011,  // (5 - sqrt(5))/20  G4
                          0.276393202250021,  // 2 * G4
                          0.414589803375032,  // 3 * G4
                         -0.447213595499958); // -1 + 4 * G4

    // (sqrt(5) - 1)/4 = F4, used once below
    //#define F4 0.309016994374947451

    // First corner
    vec4 i  = floor(v + dot(v, vec4(0.309016994374947451)) );
    vec4 x0 = v -   i + dot(i, C.xxxx);

    // Other corners

    // Rank sorting originally contributed by Bill Licea-Kane, AMD (formerly ATI)
    vec4 i0;
    vec3 isX = step( x0.yzw, x0.xxx );
    vec3 isYZ = step( x0.zww, x0.yyz );
    //  i0.x = dot( isX, vec3( 1.0 ) );
    i0.x = isX.x + isX.y + isX.z;
    i0.yzw = 1.0 - isX;
    //  i0.y += dot( isYZ.xy, vec2( 1.0 ) );
    i0.y += isYZ.x + isYZ.y;
    i0.zw += 1.0 - isYZ.xy;
    i0.z += isYZ.z;
    i0.w += 1.0 - isYZ.z;

    // i0 now contains the unique values 0,1,2,3 in each channel
    vec4 i3 = clamp( i0, 0.0, 1.0 );
    vec4 i2 = clamp( i0-1.0, 0.0, 1.0 );
    vec4 i1 = clamp( i0-2.0, 0.0, 1.0 );

    //  x0 = x0 - 0.0 + 0.0 * C.xxxx
    //  x1 = x0 - i1  + 1.0 * C.xxxx
    //  x2 = x0 - i2  + 2.0 * C.xxxx
    //  x3 = x0 - i3  + 3.0 * C.xxxx
    //  x4 = x0 - 1.0 + 4.0 * C.xxxx
    vec4 x1 = x0 - i1 + C.xxxx;
    vec4 x2 = x0 - i2 + C.yyyy;
    vec4 x3 = x0 - i3 + C.zzzz;
    vec4 x4 = x0 + C.wwww;

    // Permutations
    i = _noise_mod289(i);
    float j0 = _noise_permute( _noise_permute( _noise_permute( _noise_permute(i.w) + i.z) + i.y) + i.x);
    vec4 j1 = _noise_permute( _noise_permute( _noise_permute( _noise_permute (
               i.w + vec4(i1.w, i2.w, i3.w, 1.0 ))
             + i.z + vec4(i1.z, i2.z, i3.z, 1.0 ))
             + i.y + vec4(i1.y, i2.y, i3.y, 1.0 ))
             + i.x + vec4(i1.x, i2.x, i3.x, 1.0 ));

    // Gradients: 7x7x6 points over a cube, mapped onto a 4-cross polytope
    // 7*7*6 = 294, which is close to the ring size 17*17 = 289.
    vec4 ip = vec4(1.0/294.0, 1.0/49.0, 1.0/7.0, 0.0) ;

    vec4 p0 = _noise_grad4(j0,   ip);
    vec4 p1 = _noise_grad4(j1.x, ip);
    vec4 p2 = _noise_grad4(j1.y, ip);
    vec4 p3 = _noise_grad4(j1.z, ip);
    vec4 p4 = _noise_grad4(j1.w, ip);

    // Normalise gradients
    vec4 norm = _noise_taylorInvSqrt(vec4(dot(p0,p0), dot(p1,p1), dot(p2, p2), dot(p3,p3)));
    p0 *= norm.x;
    p1 *= norm.y;
    p2 *= norm.z;
    p3 *= norm.w;
    p4 *= _noise_taylorInvSqrt(dot(p4,p4));

    // Mix contributions from the five corners
    vec3 m0 = max(0.6 - vec3(dot(x0,x0), dot(x1,x1), dot(x2,x2)), 0.0);
    vec2 m1 = max(0.6 - vec2(dot(x3,x3), dot(x4,x4)            ), 0.0);
    m0 = m0 * m0;
    m1 = m1 * m1;
    return 49.0 * ( dot(m0*m0, vec3( dot( p0, x0 ), dot( p1, x1 ), dot( p2, x2 )))
                 + dot(m1*m1, vec2( dot( p3, x3 ), dot( p4, x4 ) ) ) ) ;
}

float noise(vec2 v, int levels, float persistence) {
      float result = 0.0, amp = 1.0, freq = 1.0, total = 0.0;
      for (int i = 0; i < levels; i++) {
          result += amp * noise(v * freq);
          total += amp;
          amp *= persistence;
          freq *= 2.0;
      }
      return result / total;
}

float noise(vec3 v, int levels, float persistence) {
      float result = 0.0, amp = 1.0, freq = 1.0, total = 0.0;
      for (int i = 0; i < levels; i++) {
          result += amp * noise(v * freq);
          total += amp;
          amp *= persistence;
          freq *= 2.0;
      }
      return result / total;
}

float noise(vec4 v, int levels, float persistence) {
      float result = 0.0, amp = 1.0, freq = 1.0, total = 0.0;
      for (int i = 0; i < levels; i++) {
          result += amp * noise(v * freq);
          total += amp;
          amp *= persistence;
          freq *= 2.0;
      }
      return result / total;
}

float noise_grad(vec3 v, out vec3 gradient) {
    const vec2  C = vec2(1.0/6.0, 1.0/3.0) ;
    const vec4  D = vec4(0.0, 0.5, 1.0, 2.0);

    // First corner
    vec3 i  = floor(v + dot(v, C.yyy) );
    vec3 x0 =   v - i + dot(i, C.xxx) ;

    // Other corners
    vec3 g = step(x0.yzx, x0.xyz);
    vec3 l = 1.0 - g;
    vec3 i1 = min( g.xyz, l.zxy );
    vec3 i2 = max( g.xyz, l.zxy );

    //   x0 = x0 - 0.0 + 0.0 * C.xxx;
    //   x1 = x0 - i1  + 1.0 * C.xxx;
    //   x2 = x0 - i2  + 2.0 * C.xxx;
    //   x3 = x0 - 1.0 + 3.0 * C.xxx;
    vec3 x1 = x0 - i1 + C.xxx;
    vec3 x2 = x0 - i2 + C.yyy; // 2.0*C.x = 1/3 = C.y
    vec3 x3 = x0 - D.yyy;      // -1.0+3.0*C.x = -0.5 = -D.y

    // Permutations
    i = _noise_mod289(i);
    vec4 p = _noise_permute( _noise_permute( _noise_permute(
               i.z + vec4(0.0, i1.z, i2.z, 1.0 ))
             + i.y + vec4(0.0, i1.y, i2.y, 1.0 ))
             + i.x + vec4(0.0, i1.x, i2.x, 1.0 ));

    // Gradients: 7x7 points over a square, mapped onto an octahedron.
    // The ring size 17*17 = 289 is close to a multiple of 49 (49*6 = 294)
    float n_ = 0.142857142857; // 1.0/7.0
    vec3  ns = n_ * D.wyz - D.xzx;

    vec4 j = p - 49.0 * floor(p * ns.z * ns.z);  //  mod(p,7*7)

    vec4 x_ = floor(j * ns.z);
    vec4 y_ = floor(j - 7.0 * x_ );    // mod(j,N)

    vec4 x = x_ *ns.x + ns.yyyy;
    vec4 y = y_ *ns.x + ns.yyyy;
    vec4 h = 1.0 - abs(x) - abs(y);

    vec4 b0 = vec4( x.xy, y.xy );
    vec4 b1 = vec4( x.zw, y.zw );

    //vec4 s0 = vec4(lessThan(b0,0.0))*2.0 - 1.0;
    //vec4 s1 = vec4(lessThan(b1,0.0))*2.0 - 1.0;
    vec4 s0 = floor(b0)*2.0 + 1.0;
    vec4 s1 = floor(b1)*2.0 + 1.0;
    vec4 sh = -step(h, vec4(0.0));

    vec4 a0 = b0.xzyw + s0.xzyw*sh.xxyy ;
    vec4 a1 = b1.xzyw + s1.xzyw*sh.zzww ;

    vec3 p0 = vec3(a0.xy,h.x);
    vec3 p1 = vec3(a0.zw,h.y);
    vec3 p2 = vec3(a1.xy,h.z);
    vec3 p3 = vec3(a1.zw,h.w);

    //Normalise gradients
    vec4 norm = _noise_taylorInvSqrt(vec4(dot(p0,p0), dot(p1,p1), dot(p2, p2), dot(p3,p3)));
    p0 *= norm.x;
    p1 *= norm.y;
    p2 *= norm.z;
    p3 *= norm.w;

    // Mix final noise value
    vec4 m = max(0.6 - vec4(dot(x0,x0), dot(x1,x1), dot(x2,x2), dot(x3,x3)), 0.0);
    vec4 m2 = m * m;
    vec4 m4 = m2 * m2;
    vec4 pdotx = vec4(dot(p0,x0), dot(p1,x1), dot(p2,x2), dot(p3,x3));

    // Determine noise gradient
    vec4 temp = m2 * m * pdotx;
    gradient = -8.0 * (temp.x * x0 + temp.y * x1 + temp.z * x2 + temp.w * x3);
    gradient += m4.x * p0 + m4.y * p1 + m4.z * p2 + m4.w * p3;
    gradient *= 42.0;

    return 42.0 * dot(m4, pdotx);
}

vec3 _noise_random3(vec3 pos)
{
    // get random value for this point
    mat3 verschwurbel = mat3( -0.865108, 0.458987, 0.202286,
                              0.46632, 0.884525, -0.0126945,
                              -0.184754, 0.0833479, -0.979244);
    vec3 randomizedPoint = verschwurbel * pos;
    return fract(cos (randomizedPoint * 17.014686 + vec3(17.0, 23.0, 51.0))*31718.927);
}

float _noise_random1(vec3 pos)
{
    return fract(cos(pos.x * 4141.0 + pos.y * 2326.0 + pos.z * 5771.0) * 198237.0);
}

/// Coefficients for stretching the UV-coords of the sphere-ified cube, so that the (skewed) squares have a uniform size
#define PLATE_STRETCH 0.8
#define PLATE_STRETCH_ASIN asin(PLATE_STRETCH)

vec3 plate_unstretch(vec3 v) {
    return sin(v * PLATE_STRETCH_ASIN) / PLATE_STRETCH;
}

/*
  GOAL:

  to find a worley noise function that can be
    a. evaluated given cubic coords (x,y,z), finding the closest grid point, and
    b. iterated over all grid points on the surface of the cube

  it acts on a cube square size of NxN, and densities for graphical features can be
  set by only iterating over multiples of M.

 */
vec3 _shifted_grid_pos(vec3 intPos, float density)
{
    vec3 rdm = _noise_random3(intPos) * 0.5 - vec3(0.25);

    // check dimensionality for this grid point and apply offset
    vec3 dimensionality = step(abs(intPos), vec3(density - 0.5));
    return intPos + rdm * dimensionality;
}

vec2 worley(vec3 sphericalPos, float density)
{
    // find axis with maximum extent
    vec3 absPos = abs(sphericalPos);
    vec3 sgnPos = sign(sphericalPos);
    float maxElem = max(absPos.x, max(absPos.y, absPos.z));
    vec3 mainAxis = step(vec3(maxElem), absPos);
    vec3 sideAxis = vec3(1.0) - mainAxis;

    vec3 cubicPos = mainAxis * sgnPos + sideAxis * plate_unstretch(sphericalPos / maxElem);
    vec3 ofs1 = mainAxis.yzx;
    vec3 ofs2 = mainAxis.zxy;

    // each grid point can be classified as being 0-, 1-, or 2-dimensional.
    //   0-dimensional: corner (three dimensions are +/- 1.0)
    //   1-dimensional: edge (two dimensions are +/- 1.0)
    //   2-dimensional: square-face (one dimension is +/- 1.0)
    // the grid point may be shifted across exactly the number of dimensions.

    // we find out, for the given cubic position, which face this position is on.
    // then we know which 4 grid points to check (the 4 neighbors on that face)

    vec3 noisePos = cubicPos * density;
    vec3 basePos = floor(noisePos);

    // now we check where exactly those 4 neighbor grid points are
    vec3 gridPos0 = _shifted_grid_pos(basePos, density);
    vec3 gridPos1 = _shifted_grid_pos(basePos + ofs1, density);
    vec3 gridPos2 = _shifted_grid_pos(basePos + ofs2, density);
    vec3 gridPos3 = _shifted_grid_pos(basePos + ofs1 + ofs2, density);

    // get min. distance to neighboring grid points
    vec4 dists2 = vec4(
        dot(noisePos - gridPos0, noisePos - gridPos0),
        dot(noisePos - gridPos1, noisePos - gridPos1),
        dot(noisePos - gridPos2, noisePos - gridPos2),
        dot(noisePos - gridPos3, noisePos - gridPos3)
    );

    float minDist2 = min(dists2.x, min(dists2.y, min(dists2.z, dists2.w)));
    dists2 += step(vec4(minDist2), dists2);
    float minDist22 = min(dists2.x, min(dists2.y, min(dists2.z, dists2.w)));

    return sqrt(vec2(minDist2, minDist22));
}

float cubicPerlinNoise(vec3 cubicPos, mat3 offsets, float density)
{
    vec3 p = cubicPos * density;
    vec3 pi = floor(p);
    vec3 pf = fract(p);

    // get random values for these 4 neighbors and interpolate
    float r00 = _noise_random1(pi);
    float r01 = _noise_random1(pi + offsets[0]);
    float r10 = _noise_random1(pi + offsets[1]);
    float r11 = _noise_random1(pi + offsets[2]);

    // lerp!
    float dx = dot(pf * offsets[1], vec3(1.0));
    float dy = dot(pf * offsets[0], vec3(1.0));

    float vx0 = mix(r00, r10, smoothstep(0.0, 1.0, dx));
    float vx1 = mix(r01, r11, smoothstep(0.0, 1.0, dx));
    return mix(vx0, vx1, smoothstep(0.0, 1.0, dy));
}")
    }

    pub fn declarations() -> String {
        String::from("float noise(vec2 v);
float noise(vec3 v);
float noise(vec4 v);
float noise(vec2 v, int levels, float persistence);
float noise(vec3 v, int levels, float persistence);
float noise(vec4 v, int levels, float persistence);
float noise_grad(vec3 v, out vec3 gradient);
vec2 worley(vec3 cubicPos, float density);
float cubicPerlinNoise(vec3 cubicPos, mat3 offsets, float density);
")
    }
}