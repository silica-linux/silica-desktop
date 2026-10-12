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

// Dimensione della texture (sfondo già sfocato) in pixel fisici, e la regione dentro la
// texture: origine e dimensione, con raggio ed esponente degli angoli.
uniform vec2 tex_size;
uniform vec4 region;
uniform float radius;
uniform float exponent;
// Colore del materiale sui valori sRGB codificati: out = M · in + b (righe di M).
uniform vec3 row_r;
uniform vec3 row_g;
uniform vec3 row_b;
uniform vec3 offset;

float shape_distance(vec2 p, vec4 rect) {
    vec2 half_size = rect.zw * 0.5;
    vec2 d = abs(p - (rect.xy + half_size)) - (half_size - vec2(radius));
    if (radius > 0.0 && d.x > 0.0 && d.y > 0.0) {
        vec2 q = d / radius;
        float f = pow(pow(q.x, exponent) + pow(q.y, exponent), 1.0 / exponent);
        return (f - 1.0) * radius;
    }
    return max(d.x, d.y) - (radius > 0.0 ? radius : 0.0);
}

void main() {
    vec2 p = v_coords * tex_size;
    vec3 background = texture2D(tex, v_coords).rgb;
    vec3 color = clamp(vec3(dot(row_r, background), dot(row_g, background), dot(row_b, background)) + offset, 0.0, 1.0);
    float coverage = clamp(0.5 - shape_distance(p, region), 0.0, 1.0) * alpha;
    vec4 result = vec4(color, 1.0) * coverage;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        result = vec4(0.0, 0.2, 0.0, 0.2) + result * 0.8;
#endif
    gl_FragColor = result;
}
