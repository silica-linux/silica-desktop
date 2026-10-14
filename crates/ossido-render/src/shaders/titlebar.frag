precision highp float;

uniform float alpha;
uniform vec2 size;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

// In logical points. The bar is the top part of the frame (bar + content): the top
// corners follow the window superellipse.
uniform float frame_height;
uniform float radius;
uniform float exponent;
// Points per physical pixel, for antialiasing.
uniform float pixel;
uniform vec3 background;
// Separator row at the bottom of the bar (alpha 0 if absent).
uniform vec4 separator;
// 1 = active window (colored buttons), 0 = inactive (gray).
uniform float active;
// Opacity of the symbols (pointer on the group); above 0 the colors are the active ones even
// in an inactive window.
uniform float symbols;
// Pressed button: 0, 1, 2; -1 none.
uniform float pressed;
// 1 = ⌥ held: "+" on the green one.
uniform float alt;
// 1 = full screen disabled (fixed-size window), 2 = minimize and full screen disabled (dialog):
// gray discs without a symbol.
uniform float disabled;
// 0 = full-size content: no background or row, only the buttons over the app.
uniform float bar;
// Center of the first button (the others are 23 pt apart) and the vertical center of all.
uniform float button_x;
uniform float button_y;
// 1 = dark appearance (windows.md: "Dark" tables of the buttons).
uniform float dark;

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

// Coverage of a region given its signed distance (negative inside), in points.
float cover(float distance) {
    return clamp(0.5 - distance / pixel, 0.0, 1.0);
}

vec4 over(vec4 top, vec4 bottom) {
    return top + bottom * (1.0 - top.a);
}

// Color of a button: vertical gradient (y 20 → 26 → 31 of the measured crop) and 1 pt
// outline (windows.md, "Window buttons").
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

// Symbols when the pointer passes, centered on the button.
float symbol_close(vec2 d) {
    // X of 8 × 8 pt, 2 pt strokes.
    float a = abs(d.x - d.y) / 1.41421356;
    float b = abs(d.x + d.y) / 1.41421356;
    float stroke = min(a, b) - 1.0;
    float box = max(abs(d.x), abs(d.y)) - 4.0;
    return cover(max(stroke, box));
}

float symbol_minimize(vec2 d) {
    // Dash of 8 × 2 pt.
    return cover(max(abs(d.x) - 4.0, abs(d.y) - 1.0));
}

float symbol_plus(vec2 d) {
    // "+" of 8 × 8 pt, 1.6 pt strokes (⌥: zoom).
    float h = max(abs(d.x) - 4.0, abs(d.y) - 0.8);
    float v = max(abs(d.y) - 4.0, abs(d.x) - 0.8);
    return cover(min(h, v));
}

float symbol_zoom(vec2 d) {
    // Two opposite right triangles in 6 × 6 pt, separated by a light diagonal.
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

    // Disc colors: center, top, bottom, outline on the sides, on top and at the bottom.
    vec3 close_c, close_t, close_b, close_o, close_ot, close_ob;
    vec3 min_c, min_t, min_b, min_o, min_ot, min_ob;
    vec3 zoom_c, zoom_t, zoom_b, zoom_o, zoom_ot, zoom_ob;
    // Grays of the inactive ones, also for the disabled buttons.
    vec3 grey_c, grey_t, grey_b, grey_o, grey_ot, grey_ob;
    bool lit = active > 0.5 || symbols > 0.0;
    bool is_dark = dark > 0.5;
    if (is_dark) {
        // Dark: luminous outline on top and at the bottom.
        close_c = vec3(0.949, 0.376, 0.337); close_t = vec3(0.973, 0.388, 0.353); close_b = vec3(0.933, 0.400, 0.376);
        close_o = vec3(0.933, 0.333, 0.286); close_ot = vec3(0.980, 0.694, 0.655); close_ob = vec3(0.984, 0.678, 0.647);
        min_c = vec3(0.992, 0.753, 0.008); min_t = vec3(0.980, 0.710, 0.000); min_b = vec3(0.992, 0.804, 0.161);
        min_o = vec3(0.973, 0.725, 0.016); min_ot = vec3(0.984, 0.933, 0.353); min_ob = vec3(0.984, 0.973, 0.412);
        zoom_c = vec3(0.224, 0.757, 0.004); zoom_t = vec3(0.004, 0.737, 0.000); zoom_b = vec3(0.294, 0.757, 0.200);
        zoom_o = vec3(0.035, 0.725, 0.004); zoom_ot = vec3(0.588, 0.984, 0.471); zoom_ob = vec3(0.545, 0.984, 0.463);
        // Inactive: outline only on top and at the bottom, none on the sides.
        grey_c = vec3(0.302, 0.302, 0.302); grey_t = vec3(0.322, 0.322, 0.325); grey_b = vec3(0.282, 0.282, 0.286);
        grey_o = grey_c; grey_ot = vec3(0.600, 0.600, 0.600); grey_ob = vec3(0.565, 0.565, 0.569);
    } else {
        // Light: outline darker than the background, the same all around.
        close_c = vec3(0.973, 0.490, 0.467); close_t = vec3(0.996, 0.431, 0.400); close_b = vec3(0.961, 0.545, 0.514);
        close_o = vec3(0.867, 0.302, 0.278); close_ot = close_o; close_ob = close_o;
        min_c = vec3(0.988, 0.788, 0.231); min_t = vec3(0.988, 0.722, 0.180); min_b = vec3(0.996, 0.827, 0.267);
        min_o = vec3(0.816, 0.553, 0.043); min_ot = min_o; min_ob = min_o;
        zoom_c = vec3(0.522, 0.839, 0.376); zoom_t = vec3(0.384, 0.816, 0.180); zoom_b = vec3(0.600, 0.851, 0.475);
        zoom_o = vec3(0.318, 0.620, 0.196); zoom_ot = zoom_o; zoom_ob = zoom_o;
        grey_c = vec3(0.918, 0.922, 0.925); grey_t = vec3(0.925, 0.929, 0.933); grey_b = vec3(0.925, 0.922, 0.941);
        grey_o = vec3(0.765, 0.765, 0.780); grey_ot = vec3(0.902, 0.906, 0.910); grey_ob = vec3(0.890, 0.890, 0.906);
    }
    if (!lit) {
        close_c = grey_c; close_t = grey_t; close_b = grey_b;
        close_o = grey_o; close_ot = grey_ot; close_ob = grey_ob;
        min_c = grey_c; min_t = grey_t; min_b = grey_b;
        min_o = grey_o; min_ot = grey_ot; min_ob = grey_ob;
        zoom_c = grey_c; zoom_t = grey_t; zoom_b = grey_b;
        zoom_o = grey_o; zoom_ot = grey_ot; zoom_ob = grey_ob;
    }
    bool zoom_off = disabled > 0.5;
    bool min_off = disabled > 1.5;
    // Pressed: the disc gets lighter (measured pressed center minus hover).
    vec3 no_shift = vec3(0.0);
    vec3 close_p = is_dark ? vec3(14.0, 31.0, 19.0) / 255.0 : vec3(6.0, 35.0, 28.0) / 255.0;
    vec3 min_p = is_dark ? vec3(2.0, 50.0, 18.0) / 255.0 : vec3(0.0, 45.0, 28.0) / 255.0;
    vec3 zoom_p = is_dark ? vec3(4.0, 8.0, 10.0) / 255.0 : vec3(1.0, 6.0, -9.0) / 255.0;
    vec3 close_s = pressed == 0.0 ? close_p : no_shift;
    vec3 min_s = pressed == 1.0 && !min_off ? min_p : no_shift;
    vec3 zoom_s = pressed == 2.0 && !zoom_off ? zoom_p : no_shift;

    vec2 c1 = vec2(button_x, button_y);
    vec2 c2 = vec2(button_x + 23.0, button_y);
    vec2 c3 = vec2(button_x + 46.0, button_y);
    color = over(button(p, c1, close_t, close_c, close_b, close_o, close_ot, close_ob, close_s),
                 color);
    if (min_off) {
        color = over(button(p, c2, grey_t, grey_c, grey_b, grey_o, grey_ot, grey_ob, no_shift), color);
    } else {
        color = over(button(p, c2, min_t, min_c, min_b, min_o, min_ot, min_ob, min_s), color);
    }
    if (zoom_off) {
        color = over(button(p, c3, grey_t, grey_c, grey_b, grey_o, grey_ot, grey_ob, no_shift), color);
    } else {
        color = over(button(p, c3, zoom_t, zoom_c, zoom_b, zoom_o, zoom_ot, zoom_ob, zoom_s), color);
    }

    if (symbols > 0.0) {
        // Symbols: lighter and more saturated on the pressed button.
        vec3 close_y, min_y, zoom_y;
        if (is_dark) {
            close_y = pressed == 0.0 ? vec3(0.733, 0.000, 0.000) : vec3(0.549, 0.000, 0.000);
            min_y = pressed == 1.0 ? vec3(0.867, 0.298, 0.000) : vec3(0.647, 0.227, 0.000);
            zoom_y = pressed == 2.0 ? vec3(0.200, 0.494, 0.000) : vec3(0.176, 0.447, 0.004);
        } else {
            close_y = pressed == 0.0 ? vec3(0.855, 0.000, 0.000) : vec3(0.639, 0.000, 0.000);
            min_y = pressed == 1.0 ? vec3(1.000, 0.353, 0.000) : vec3(0.757, 0.267, 0.000);
            zoom_y = pressed == 2.0 ? vec3(0.231, 0.588, 0.000) : vec3(0.212, 0.533, 0.000);
        }
        color = over(vec4(close_y, 1.0) * symbol_close(p - c1) * symbols, color);
        if (!min_off) {
            color = over(vec4(min_y, 1.0) * symbol_minimize(p - c2) * symbols, color);
        }
        if (!zoom_off) {
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
