uniform float planetRadius;
uniform float waterLevel;
uniform vec3 eyePosition;
uniform vec3 sunDirection;
uniform float inScatterFac;
uniform float waterSeed;
uniform float angleToHorizon;
uniform float terrainMaxHeight;
uniform mat4 inverseViewProjectionMatrix;
uniform sampler2D planetColor;
uniform sampler2D planetNormal;
uniform sampler2D planetPosition;

$SHADOWS
$NOISE
$ATMOSPHERE

in vec2 clipPos;
out vec4 outColor;

uniform vec3 white_point;
uniform float exposure;
uniform vec2 sun_size;

float planetRadiusIntersect(vec3 r0, vec3 rd, float radius)
{
    float a = dot(rd, rd);
    float b = 2.0 * dot(rd, r0);
    float c = dot(r0, r0) - (radius * radius);
    if (b*b - 4.0*a*c < 0.0)
        return -1.0;
    return (-b - sqrt((b*b) - 4.0*a*c))/(2.0*a);
}

void main() {
    vec4 normalFromTex = texture(planetNormal, vec2(0.5) + 0.5 * clipPos);

    // calculate eye direction in that pixel
    vec4 globalPosV4 = inverseViewProjectionMatrix * vec4(clipPos, 0.0, 1.0);
    vec3 globalPos = globalPosV4.xyz / globalPosV4.w;
    vec3 eyeDir = normalize(globalPos - eyePosition);

    // TODO: if we are underwater, do something different

    //
    // Calculate color of the sky, if we are not looking at the earth
    //
    if (length(normalFromTex) <= 0.0) {
        // Compute the radiance of the sky.
        float shadow_length = 0.0;
        vec3 transmittance;
        vec3 radiance = GetSkyRadiance(
            eyePosition / planetRadius, eyeDir, shadow_length, sunDirection, transmittance
        );

        // If the view ray intersects the Sun, add the Sun radiance.
        if (dot(eyeDir, sunDirection) > sun_size.y) {
            radiance = radiance + transmittance * GetSolarRadiance();
        }
        outColor = vec4(pow(vec3(1.0) - exp(-radiance / white_point * exposure), vec3(1.0 / 2.2)), 1.0);
        return;
    }

    vec3 color = vec3(0.0);

    //
    // Load position/color from planet rendering textures
    //
    float wireframe = normalFromTex.w;
    vec4 pColorReflectivity = texture(planetColor, vec2(0.5) + 0.5 * clipPos);
    vec3 pColor = pColorReflectivity.rgb;
    vec4 pPosHeight = texture(planetPosition, vec2(0.5) + 0.5 * clipPos);
    vec3 vertical = normalize(pPosHeight.xyz);
    float eyeToTerrainDist = length(pPosHeight.xyz - eyePosition);
    vec3 actualSurfaceNormal = vec3(-1.0) + 2.0 * normalFromTex.xyz;

    // correct normal to be vertical for water (only for shadow computation for now)
    if (pColorReflectivity.a > 0.0)
        actualSurfaceNormal = vertical;

    //
    // Find out if we are shadowed by the terrain, and interpolate between last and curr sun position
    //
    float dotSun = dot(actualSurfaceNormal, sunDirection);
    vec3 shadowMapDebugColor;
    float shadow = getShadow(pPosHeight.xyz, dotSun, eyeToTerrainDist, shadowMapDebugColor);

    // visibility of the sky and sun, based on shadows cast by the terrain
    float sunVisibility = 0.5 * shadow;
    float skyVisibility = 1.0;

    vec3 atmoEyePos = eyePosition / planetRadius;
    vec3 atmoSurfPos = pPosHeight.xyz / planetRadius;

    //
    // Compute the radiance reflected by the ground.
    //
    vec3 sky_irradiance;
    vec3 sun_irradiance = GetSunAndSkyIrradiance(pPosHeight.xyz / planetRadius, normalize(pPosHeight.xyz), sunDirection, sky_irradiance);
    sky_irradiance = max(sky_irradiance, vec3(0.005, 0.008, 0.01));
    vec3 irradiance = sun_irradiance * sunVisibility + sky_irradiance * skyVisibility;
    vec3 ground_radiance = pColor * (1.0 / PI) * irradiance;

    //
    // Incorporate reflectivity for water rendering
    //
    if (pColorReflectivity.a > 0.0) {
        vec3 viewDirection = (pPosHeight.xyz - eyePosition) / eyeToTerrainDist;
        float shininess = 100.0;
        vec3 sunlight = vec3(0.008, 0.008, 0.006);

        // add some distortion to water normal
        // the amount of distortion depends on the viewing distance and angle,
        // in order to not create pixely glitter artifacts

        for (int i = 0; i <= 3; ++i) {
            vec3 grad;
            float glitter_scale = 150.0 * pow(4.0, float(i));
            noise_grad(pPosHeight.xyz * glitter_scale + vertical * waterSeed, grad);
            float glitter_angle = max(-dot(viewDirection, actualSurfaceNormal), 0.0);
            float glitter_dist = 1.0 / eyeToTerrainDist;
            float glitter = min(15.0 * glitter_angle * glitter_dist / glitter_scale, 1.0);
            actualSurfaceNormal += grad * glitter * 0.2;
        }

        actualSurfaceNormal = normalize(actualSurfaceNormal);

        // compute specular highlight
        vec3 reflectedSunlight = reflect(sunDirection, actualSurfaceNormal);
        float nf = (shininess + 2.0) / 2.0;
        float waterSpecularity = nf * pow(max(dot(reflectedSunlight, viewDirection), 0.0), shininess);
        ground_radiance += waterSpecularity * pColorReflectivity.a * sunlight * (sun_irradiance + sky_irradiance);
    }

    // float shadow_length =
    //     max(0.0, min(shadow_out, distance_to_intersection) - shadow_in) *
    //     lightshaft_fadein_hack;
    float shadow_length = 0.0;

    // if we are looking 'up', i.e. our view ray doesn't intersect the normalized
    // planet sphere, we'll have to adjust that because otherwise the in-scatter
    // light will look shitty. we'll also have to make sure that our terrain heights
    // are clipped at the atmosphere boundary.
    float ES = length(atmoEyePos);
    float EP = length(atmoEyePos - atmoSurfPos);
    float SP = length(atmoSurfPos);
    float angleToView = acos((ES*ES + EP*EP - SP*SP) / (2.0 * ES * EP));
    float atmSurfRadius = SP;

    float deltaAngle = angleToView - 0.99 * angleToHorizon;
    if (deltaAngle > 0.0)
        atmSurfRadius = SP - tan(deltaAngle) * EP;
    atmSurfRadius = min(atmSurfRadius, 0.9999 * terrainMaxHeight);
    atmoSurfPos *= atmSurfRadius / SP;

    // compute transmittance of the original terrain color + in-scattering of the sun
    vec3 transmittance;
    vec3 in_scatter = GetSkyRadianceToPoint(atmoEyePos, atmoSurfPos, shadow_length, sunDirection, transmittance);
    ground_radiance *= transmittance;
    ground_radiance += in_scatter * inScatterFac;

    // do final color mapping
    color = pow(vec3(1.0) - exp(-ground_radiance / white_point * exposure), vec3(1.0 / 2.2));

    // draw wireframes on top?
    float brightness = dot(vec3(0.2126, 0.7152, 0.0722), color);
    color = mix(color, vec3(step(brightness, 0.4)), wireframe);

    outColor = vec4(color, 1.0);
}
