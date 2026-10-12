precision highp float;

uniform float alpha;
uniform vec2 size;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

// Tutto in punti logici, relativo all'area dell'elemento.
uniform vec4 window_rect;
uniform float radius;
uniform float exponent;
// Profilo misurato: alfa(d) = amplitude · ½·erfc((d − shift) / (√2·sigma)), con l'ombra
// spostata in basso di offset_y.
uniform float offset_y;
uniform float amplitude;
uniform float shift;
uniform float sigma;
// Anello di 1 pt a contatto con la finestra.
uniform float ring_side;
uniform float ring_top;

// Distanza con segno (positiva fuori) da un rettangolo con angoli a superellisse.
float shape_distance(vec2 p, vec4 rect) {
    vec2 half_size = rect.zw * 0.5;
    vec2 d = abs(p - (rect.xy + half_size)) - (half_size - vec2(radius));
    if (d.x > 0.0 && d.y > 0.0) {
        vec2 q = d / radius;
        float f = pow(pow(q.x, exponent) + pow(q.y, exponent), 1.0 / exponent);
        return (f - 1.0) * radius;
    }
    return max(d.x, d.y) - radius;
}

// erfc con l'approssimazione 7.1.26 di Abramowitz e Stegun (errore < 1,5e-7).
float erfc_approx(float x) {
    float z = abs(x);
    float t = 1.0 / (1.0 + 0.3275911 * z);
    float poly = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741
        + t * (-1.453152027 + t * 1.061405429))));
    float e = poly * exp(-z * z);
    return x >= 0.0 ? e : 2.0 - e;
}

void main() {
    vec2 p = v_coords * size;
    float body = shape_distance(p, window_rect);
    // d = 1 è il primo pixel dopo l'anello: il centro di quel pixel è a 1,5 dal bordo.
    vec4 shadow_rect = window_rect + vec4(0.0, offset_y, 0.0, 0.0);
    float d = shape_distance(p, shadow_rect) - 0.5;
    float shadow = amplitude * 0.5 * erfc_approx((d - shift) / (1.41421356 * sigma));

    float ring = p.y < window_rect.y ? ring_top : ring_side;
    float ring_coverage = clamp(body + 0.5, 0.0, 1.0) * clamp(1.5 - body, 0.0, 1.0);
    float window_coverage = clamp(0.5 - body, 0.0, 1.0);
    float a = max(shadow, ring * ring_coverage) * (1.0 - window_coverage) * alpha;

    vec4 color = vec4(0.0, 0.0, 0.0, a);
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
