precision highp float;

uniform float alpha;
uniform vec2 size;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

// In logical points; the element covers the window frame.
uniform float radius;
uniform float exponent;
// Points per physical pixel, for antialiasing.
uniform float pixel;
// Colors of the top row, of the sides and of the bottom row.
uniform vec3 top;
uniform vec3 side;
uniform vec3 bottom;

// Signed distance (positive outside) from the window's superellipse outline.
float shape_distance(vec2 p) {
    vec2 half_size = size * 0.5;
    vec2 d = abs(p - half_size) - (half_size - vec2(radius));
    if (d.x > 0.0 && d.y > 0.0) {
        vec2 q = d / radius;
        float f = pow(pow(q.x, exponent) + pow(q.y, exponent), 1.0 / exponent);
        return (f - 1.0) * radius;
    }
    return max(d.x, d.y) - radius;
}

float cover(float distance) {
    return clamp(0.5 - distance / pixel, 0.0, 1.0);
}

void main() {
    vec2 p = v_coords * size;
    float d = shape_distance(p);
    // The 1 pt band just inside the outline.
    float band = cover(d) - cover(d + 1.0);
    // The nearest side chooses the color.
    float left_right = min(p.x, size.x - p.x);
    vec3 color = side;
    if (p.y < left_right && p.y <= size.y - p.y) {
        color = top;
    } else if (size.y - p.y < left_right) {
        color = bottom;
    }
    vec4 result = vec4(color, 1.0) * band * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        result = vec4(0.0, 0.2, 0.0, 0.2) + result * 0.8;
#endif
    gl_FragColor = result;
}
