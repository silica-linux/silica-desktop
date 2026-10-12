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
// texture: origine e dimensione, con raggio ed esponente degli angoli (2: i vetri misurati
// hanno angoli ad arco di cerchio).
uniform vec2 tex_size;
uniform vec4 region;
uniform float radius;
uniform float exponent;
// Colore del materiale sui valori sRGB codificati: out = M · in + b (righe di M).
uniform vec3 row_r;
uniform vec3 row_g;
uniform vec3 row_b;
uniform vec3 offset;
// Pixel fisici per punto.
uniform float scale_px;
// Rifrazione: ampiezza in pixel ai lati, sopra e sotto; distanza dal bordo del massimo e
// della fine dello spostamento (pixel).
uniform vec3 refraction;
uniform vec2 refraction_band;
// Bordo: riga luminosa di 1 pt sopra e sotto (aggiunta per canale), sfumatura interna
// (aggiunta al primo pixel e decadimento in pixel), riga scura ai lati (alfa del nero).
uniform vec3 rim;
uniform vec2 edge;
uniform float side_dark;

float rounded_distance(vec2 p, vec4 rect, float r) {
    vec2 half_size = rect.zw * 0.5;
    r = min(r, min(half_size.x, half_size.y));
    vec2 d = abs(p - (rect.xy + half_size)) - (half_size - vec2(r));
    if (r > 0.0 && d.x > 0.0 && d.y > 0.0) {
        vec2 q = d / r;
        float f = pow(pow(q.x, exponent) + pow(q.y, exponent), 1.0 / exponent);
        return (f - 1.0) * r;
    }
    return max(d.x, d.y) - max(r, 0.0);
}

float shape_distance(vec2 p, vec4 rect) {
    return rounded_distance(p, rect, radius);
}

void main() {
    vec2 p = v_coords * tex_size;
    float sd = shape_distance(p, region);
    float t = -sd;
    // Normale verso l'esterno dalla distanza (differenze centrali), su una forma con angoli
    // almeno ampi quanto la fascia della rifrazione: senza, sulla diagonale di un angolo
    // retto la normale salta e lo spostamento lascia una cucitura.
    vec2 e = vec2(0.5, 0.0);
    float rn = max(radius, refraction_band.y);
    vec2 n = vec2(rounded_distance(p + e.xy, region, rn) - rounded_distance(p - e.xy, region, rn),
                  rounded_distance(p + e.yx, region, rn) - rounded_distance(p - e.yx, region, rn));
    n = length(n) > 0.0 ? normalize(n) : vec2(0.0, -1.0);
    float vertical = n.y * n.y;

    // Rifrazione: vicino al bordo il vetro preleva lo sfondo più verso l'interno, come
    // una lente (glass.md, "Rifrazione").
    float amplitude = mix(refraction.x, n.y < 0.0 ? refraction.y : refraction.z, vertical);
    float rise = clamp(t / max(refraction_band.x, 1e-3), 0.0, 1.0);
    float fall = clamp((t - refraction_band.x) / max(refraction_band.y - refraction_band.x, 1e-3), 0.0, 1.0);
    float displacement = amplitude * (t < refraction_band.x ? rise : 0.5 + 0.5 * cos(3.14159265 * fall));
    vec2 sample_at = (p - n * displacement) / tex_size;

    vec3 background = texture2D(tex, sample_at).rgb;
    vec3 color = vec3(dot(row_r, background), dot(row_g, background), dot(row_b, background)) + offset;

    // Bordo: 1 pt chiaro sopra e sotto con la sfumatura interna, 1 pt scuro ai lati.
    float on_rim = clamp(scale_px + 0.5 - t, 0.0, 1.0);
    float glow = edge.x * exp(-max(t - scale_px - 0.5, 0.0) / edge.y) * (1.0 - on_rim);
    color += (rim * on_rim + vec3(glow)) * vertical;
    color = mix(color, vec3(0.0), side_dark * on_rim * (1.0 - vertical));

    color = clamp(color, 0.0, 1.0);
    float coverage = clamp(0.5 - sd, 0.0, 1.0) * alpha;
    vec4 result = vec4(color, 1.0) * coverage;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        result = vec4(0.0, 0.2, 0.0, 0.2) + result * 0.8;
#endif
    gl_FragColor = result;
}
