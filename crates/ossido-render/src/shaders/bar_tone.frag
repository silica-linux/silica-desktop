#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

// The tone of the spaces bar, applied to the wallpaper before the blur
// (plan/reference/missioncontrol.md › MC-BLUR, dark):
//   T(v) = 18 + 0.664 v − 0.000944 v² on the luminance (0–255), the chroma offsets kept;
//   light: the same curve 73 levels up (derived from #9e9fa5 over #7a7b80, not measured);
//   expanded bar: a highlight growing down the bar, +45 · s(y) · (v / 255)^2.73 with
//   s = 1 − (1 − t)^1.6, t = (y − 20) / 120 (white 125 → 170, gray 86 → 92, black stays).
uniform float light;
uniform float expansion;
// Height of the capture in points (to know y).
uniform float height_pt;

float curve(float v, float y) {
    float t = clamp((y - 20.0) / 120.0, 0.0, 1.0);
    float s = (1.0 - pow(1.0 - t, 1.6)) * expansion;
    float base = 18.0 + 0.664 * v - 0.000944 * v * v;
    return base + 73.0 * light + 45.0 * s * pow(clamp(v / 255.0, 0.0, 1.0), 2.73);
}

void main() {
    vec3 c = texture2D(tex, v_coords).rgb * 255.0;
    float y = v_coords.y * height_pt;
    float l = dot(c, vec3(0.2126, 0.7152, 0.0722));
    vec3 toned = c + (curve(l, y) - l);
    vec4 color = vec4(clamp(toned / 255.0, 0.0, 1.0), 1.0) * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
