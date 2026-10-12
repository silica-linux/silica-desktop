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

// Passo di un pixel della texture lungo l'asse della passata (x o y, in coordinate 0–1).
uniform vec2 step;
// σ della gaussiana in pixel della texture; il nucleo arriva a 3σ (al più 48 pixel).
uniform float sigma;

void main() {
    vec4 sum = vec4(0.0);
    float total = 0.0;
    float reach = min(3.0 * sigma, 48.0);
    for (int i = 0; i <= 96; i++) {
        float x = float(i) - 48.0;
        if (abs(x) <= reach) {
            float weight = exp(-(x * x) / (2.0 * sigma * sigma));
            sum += texture2D(tex, v_coords + step * x) * weight;
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
