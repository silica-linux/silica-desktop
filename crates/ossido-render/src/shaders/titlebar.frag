precision highp float;

uniform float alpha;
uniform vec2 size;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

// In punti logici. La barra è la parte alta della cornice (barra + contenuto): gli
// angoli superiori seguono la superellisse della finestra.
uniform float frame_height;
uniform float radius;
uniform float exponent;
// Punti per pixel fisico, per l'antialiasing.
uniform float pixel;
uniform vec3 background;
// Riga di separazione in fondo alla barra (alfa 0 se assente).
uniform vec4 separator;
// 1 = finestra attiva (pulsanti colorati), 0 = inattiva (grigi).
uniform float active;
// Opacità dei simboli (puntatore sul gruppo); sopra 0 i colori sono quelli attivi anche in
// una finestra inattiva.
uniform float symbols;
// Pulsante premuto: 0, 1, 2; -1 nessuno.
uniform float pressed;
// 1 = ⌥ tenuto: "+" sul verde.
uniform float alt;
// 1 = riduzione e schermo intero disattivati (dischi grigi senza simbolo).
uniform float disabled;
// 0 = contenuto a tutta finestra: niente fondo né riga, solo i pulsanti sopra l'app.
uniform float bar;
// Centro verticale dei pulsanti.
uniform float button_y;

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

// Copertura di una regione data la sua distanza con segno (negativa dentro), in punti.
float cover(float distance) {
    return clamp(0.5 - distance / pixel, 0.0, 1.0);
}

vec4 over(vec4 top, vec4 bottom) {
    return top + bottom * (1.0 - top.a);
}

// Colore di un pulsante: gradiente verticale (y 20 → 26 → 31 del ritaglio misurato) e
// contorno di 1 pt (windows.md, "Pulsanti della finestra").
vec3 gradient(float dy, vec3 top, vec3 middle, vec3 bottom) {
    return dy < 0.0 ? mix(middle, top, clamp(-dy / 6.0, 0.0, 1.0))
                    : mix(middle, bottom, clamp(dy / 5.0, 0.0, 1.0));
}

vec4 button(vec2 p, vec2 center, vec3 top, vec3 middle, vec3 bottom, vec3 outline_side,
            vec3 outline_top, vec3 outline_bottom, vec3 shift) {
    vec2 d = p - center;
    float r = length(d);
    float disc = cover(r - 7.0);
    vec3 fill = clamp(gradient(d.y, top, middle, bottom) + shift, 0.0, 1.0);
    vec3 outline = d.y < 0.0 ? mix(outline_side, outline_top, -d.y / 7.0)
                             : mix(outline_side, outline_bottom, d.y / 7.0);
    float ring = clamp(r - 6.0, 0.0, 1.0);
    return vec4(mix(fill, outline, ring), 1.0) * disc;
}

// Simboli al passaggio del puntatore, centrati sul pulsante.
float symbol_close(vec2 d) {
    // X di 8 × 8 pt, tratti di 2 pt.
    float a = abs(d.x - d.y) / 1.41421356;
    float b = abs(d.x + d.y) / 1.41421356;
    float stroke = min(a, b) - 1.0;
    float box = max(abs(d.x), abs(d.y)) - 4.0;
    return cover(max(stroke, box));
}

float symbol_minimize(vec2 d) {
    // Trattino di 8 × 2 pt.
    return cover(max(abs(d.x) - 4.0, abs(d.y) - 1.0));
}

float symbol_plus(vec2 d) {
    // "+" di 8 × 8 pt, tratti di 1,6 pt (⌥: ingrandisci).
    float h = max(abs(d.x) - 4.0, abs(d.y) - 0.8);
    float v = max(abs(d.y) - 4.0, abs(d.x) - 0.8);
    return cover(min(h, v));
}

float symbol_zoom(vec2 d) {
    // Due triangoli rettangoli opposti in 6 × 6 pt, separati da una diagonale chiara.
    float box = max(abs(d.x) - 3.0, abs(d.y) - 3.0);
    float gap = 0.75 - abs(d.x + d.y) / 1.41421356;
    return cover(max(box, gap));
}

void main() {
    vec2 p = v_coords * size;
    float shape = shape_distance(p, vec4(0.0, 0.0, size.x, frame_height));
    vec4 color = bar > 0.5 ? vec4(background, 1.0) : vec4(0.0);
    if (bar > 0.5 && separator.a > 0.0 && p.y > size.y - 1.0) {
        color = vec4(mix(background, separator.rgb, separator.a), 1.0);
    }

    vec3 close_c, close_t, close_b, close_o, min_c, min_t, min_b, min_o, zoom_c, zoom_t, zoom_b, zoom_o;
    bool lit = active > 0.5 || symbols > 0.0;
    if (lit) {
        close_c = vec3(0.973, 0.490, 0.467); close_t = vec3(0.996, 0.431, 0.400);
        close_b = vec3(0.961, 0.545, 0.514); close_o = vec3(0.867, 0.302, 0.278);
        min_c = vec3(0.988, 0.788, 0.231); min_t = vec3(0.988, 0.722, 0.180);
        min_b = vec3(0.996, 0.827, 0.267); min_o = vec3(0.816, 0.553, 0.043);
        zoom_c = vec3(0.522, 0.839, 0.376); zoom_t = vec3(0.384, 0.816, 0.180);
        zoom_b = vec3(0.600, 0.851, 0.475); zoom_o = vec3(0.318, 0.620, 0.196);
    } else {
        close_c = vec3(0.918, 0.922, 0.925); close_t = vec3(0.925, 0.929, 0.933);
        close_b = vec3(0.925, 0.922, 0.941); close_o = vec3(0.765, 0.765, 0.780);
        min_c = close_c; min_t = close_t; min_b = close_b; min_o = close_o;
        zoom_c = close_c; zoom_t = close_t; zoom_b = close_b; zoom_o = close_o;
    }
    // Contorno sopra e sotto: nei pulsanti attivi come ai lati, negli inattivi più chiaro.
    vec3 ot = lit ? close_o : vec3(0.902, 0.906, 0.910);
    vec3 ob = lit ? close_o : vec3(0.890, 0.890, 0.906);
    // Disattivati: come gli inattivi.
    vec3 grey_c = vec3(0.918, 0.922, 0.925), grey_t = vec3(0.925, 0.929, 0.933);
    vec3 grey_b = vec3(0.925, 0.922, 0.941), grey_o = vec3(0.765, 0.765, 0.780);
    vec3 grey_ot = vec3(0.902, 0.906, 0.910), grey_ob = vec3(0.890, 0.890, 0.906);
    bool off = disabled > 0.5;
    // Premuto: il disco si schiarisce (centro misurato premuto meno hover).
    vec3 no_shift = vec3(0.0);
    vec3 close_s = pressed == 0.0 ? vec3(0.024, 0.137, 0.110) : no_shift;
    vec3 min_s = pressed == 1.0 && !off ? vec3(0.0, 0.176, 0.110) : no_shift;
    vec3 zoom_s = pressed == 2.0 && !off ? vec3(0.004, 0.024, -0.035) : no_shift;

    vec2 c1 = vec2(26.0, button_y);
    vec2 c2 = vec2(49.0, button_y);
    vec2 c3 = vec2(72.0, button_y);
    color = over(button(p, c1, close_t, close_c, close_b, close_o,
                        lit ? close_o : ot, lit ? close_o : ob, close_s), color);
    if (off) {
        color = over(button(p, c2, grey_t, grey_c, grey_b, grey_o, grey_ot, grey_ob, no_shift), color);
        color = over(button(p, c3, grey_t, grey_c, grey_b, grey_o, grey_ot, grey_ob, no_shift), color);
    } else {
        color = over(button(p, c2, min_t, min_c, min_b, min_o,
                            lit ? min_o : ot, lit ? min_o : ob, min_s), color);
        color = over(button(p, c3, zoom_t, zoom_c, zoom_b, zoom_o,
                            lit ? zoom_o : ot, lit ? zoom_o : ob, zoom_s), color);
    }

    if (symbols > 0.0) {
        // Simboli: più chiari e saturi sul pulsante premuto.
        vec3 close_y = pressed == 0.0 ? vec3(0.855, 0.0, 0.0) : vec3(0.639, 0.0, 0.0);
        vec3 min_y = pressed == 1.0 ? vec3(1.0, 0.353, 0.0) : vec3(0.757, 0.267, 0.0);
        vec3 zoom_y = pressed == 2.0 ? vec3(0.231, 0.588, 0.0) : vec3(0.212, 0.533, 0.0);
        color = over(vec4(close_y, 1.0) * symbol_close(p - c1) * symbols, color);
        if (!off) {
            color = over(vec4(min_y, 1.0) * symbol_minimize(p - c2) * symbols, color);
            float zoom = alt > 0.5 ? symbol_plus(p - c3) : symbol_zoom(p - c3);
            color = over(vec4(zoom_y, 1.0) * zoom * symbols, color);
        }
    }

    color *= cover(shape) * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
