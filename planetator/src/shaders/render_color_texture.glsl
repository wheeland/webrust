uniform sampler2D $TEXNAME;

vec3 $FUNCNAME(float scale, float dropoff)
{
    // build actual scale factors for this and the two neighbor tiles, based on dropoff factor
    vec3 adjustedDists = clamp(uvDists / dropoff, vec3(0.0), vec3(1.0));
    vec2 factors = pow(vec2(1.0) - adjustedDists.xy, vec2(2.0));
    if (adjustedDists.z > 0.0) factors.y *= factors.x;

    // calculate some arbitrary, but stable gradient, so that texture filtering doesn't screw up
    vec2 globalUV = (scenePosition.xy + scenePosition.yz + scenePosition.zx) * scale;
    vec2 dUVdx = dFdx(globalUV);
    vec2 dUVdy = dFdy(globalUV);

    // calculate how much the prime value is ahead of the others
    float primeness = smoothstep(1.2, 1.6, 1.0 / max(0.001, max(factors.x, factors.y)));

    vec3 ret = textureGrad($TEXNAME, uv1 * scale, dUVdx, dUVdy).rgb;
    if (primeness > 0.0) {
        // improve on inigo quilez' algorithm a bit by stretching and invsmoothstep()ing the index
        float r = -0.1 + 1.2 * noise(uv1 * scale);
        r = (r + (r - (r * r * (3.0 - 2.0 * r))));
        float index = 4.0 * clamp(r, 0.0, 1.0);
        float i = floor(index);
        float f = fract(index);
        vec2 off1 = sin(vec2(3.0,7.0)*(i+0.0));
        vec2 off2 = sin(vec2(3.0,7.0)*(i+1.0));
        vec3 jumble1 = textureGrad($TEXNAME, uv1 * scale + off1, dUVdx, dUVdy).rgb;
        vec3 jumble2 = textureGrad($TEXNAME, uv1 * scale + off2, dUVdx, dUVdy).rgb;
        vec3 jumble = mix(jumble1, jumble2, smoothstep(0.1, 1.0, f));
        ret = mix(ret, jumble, primeness);
    }
    if (factors.x > 0.0) ret += textureGrad($TEXNAME, uv23.xy * scale, dUVdx, dUVdy).rgb * factors.x;
    if (factors.y > 0.0) ret += textureGrad($TEXNAME, uv23.zw * scale, dUVdx, dUVdy).rgb * factors.y;
    return ret / (1.0 + factors.x + factors.y);
}
