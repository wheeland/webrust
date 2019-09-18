vec3 color(vec3 normal, vec3 position, float height)
{
    vec3 absPos = abs(position);
    float maxElem = max(absPos.x, max(absPos.y, absPos.z));
    vec3 faceColor = vec3(0.7) + 0.3 * step(vec3(maxElem), absPos);

    return faceColor;
}
