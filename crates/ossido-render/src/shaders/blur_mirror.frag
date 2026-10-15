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

// One texture pixel step along the axis of the pass (x or y, in 0–1 coordinates).
uniform vec2 step;
// σ of the Gaussian in texture pixels; the kernel reaches 3σ (at most 48 pixels).
uniform float sigma;

void main() {
    vec4 sum = vec4(0.0);
    float total = 0.0;
    float reach = min(3.0 * sigma, 48.0);
    // Mirrored at the edges of the capture (the spaces bar samples only what is under it,
    // reflected at its borders: missioncontrol.md › MC-BLUR).
    for (int i = 0; i <= 96; i++) {
        float x = float(i) - 48.0;
        if (abs(x) <= reach) {
            float weight = exp(-(x * x) / (2.0 * sigma * sigma));
            vec2 c = abs(v_coords + step * x);
            c = vec2(1.0) - abs(vec2(1.0) - c);
            sum += texture2D(tex, c) * weight;
            total += weight;
        }
    }
    vec4 color = sum / max(total, 1e-6) * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
