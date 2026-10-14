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

// Size of the texture (background already blurred) in physical pixels, and the regions inside
// the texture: up to 8 rectangles (one column per rectangle: x, y, width, height), their
// radii, how many there are. The regions merge with a smooth union within `merge` pixels,
// like drops. Corner exponent: 2, the measured glasses have circular arcs.
uniform vec2 tex_size;
uniform mat4 shapes[2];
uniform mat4 radii;
uniform float shape_count;
uniform float merge;
uniform float exponent;
// Material color on the encoded sRGB values: out = M · in + b + q2 · l² + q3 · l³ (rows of M,
// l the mean of the input channels).
uniform vec3 row_r;
uniform vec3 row_g;
uniform vec3 row_b;
uniform vec3 offset;
uniform vec3 curve2;
uniform vec3 curve3;
// Physical pixels per point.
uniform float scale_px;
// Edge lens: per side (0 sides, 1 top, 2 bottom) the (t, s) points in pixels: a pixel at
// distance t from the edge samples the background at distance s (negative: outside the edge).
// Columns of every mat4: t0, s0, t1, s1. lens_reach: how far the lens reaches.
uniform mat4 lens[3];
uniform float lens_reach;
// Edge, measured: per orientation (0 top, 1 sides, 2 bottom) the offsets inside the edge,
// one per point (0–11, levels 0–1 relative to the fill on gray), and the alpha of black on
// the first two points outside (slots 12, 13). Lights (> 0) add less over lighter colors,
// shadows (< 0) multiply. `fill_gray`: the fill over the reference gray.
uniform mat4 edges[3];
uniform float fill_gray;

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

// Downward tip in the rectangle: base (its width) on the top side, at the body's edge, and a
// rounded apex touching the bottom side. A rounded isosceles triangle: apex radius ~2 pt, the
// sides' angle solved so that the rounded bottom lands on the rectangle's bottom
// (misc.md, Dock context menu: base ~22 pt, ~9 pt deep, round bottom). The triangle goes on
// into the body, so its base never shows.
float iso_triangle(vec2 p, vec2 q) {
    // Apex at the origin, opening towards +y, half-width q.x at height q.y (iquilezles).
    p.x = abs(p.x);
    vec2 a = p - q * clamp(dot(p, q) / dot(q, q), 0.0, 1.0);
    vec2 b = p - q * vec2(clamp(p.x / q.x, 0.0, 1.0), 1.0);
    float s = -sign(q.y);
    vec2 d = min(vec2(dot(a, a), s * (p.x * q.y - p.y * q.x)),
                 vec2(dot(b, b), s * (p.y - q.y)));
    return -sqrt(d.x) * sign(d.y);
}

// p in the tip's own frame: the base along y = 0 (x from 0 to `size.x`), the apex towards +y
// at depth `size.y`.
float tip_distance(vec2 p, vec2 size) {
    float ra = min(2.5 * scale_px, size.x * 0.25);
    float half_base = size.x * 0.5;
    float height = size.y;
    // Depth of the sharp apex so that the rounded bottom sits at `height`.
    float depth = height;
    for (int i = 0; i < 4; i++) {
        float sin_t = half_base / sqrt(half_base * half_base + depth * depth);
        depth = height + ra * (1.0 / max(sin_t, 0.05) - 1.0);
    }
    float tan_t = half_base / depth;
    float sin_t = half_base / sqrt(half_base * half_base + depth * depth);
    // The triangle inset by ra: apex ra / sin(t) higher; rounded back by subtracting ra.
    float apex = depth - ra / max(sin_t, 0.05);
    float reach = apex + 2.0 * height;
    vec2 local = vec2(p.x - half_base, apex - p.y);
    return iso_triangle(local, vec2(reach * tan_t, reach)) - ra;
}

// Smooth (polynomial) minimum: the bridge between two nearby shapes.
float smooth_min(float a, float b, float k) {
    if (k <= 0.0) {
        return min(a, b);
    }
    float h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// Signed distance from the union of the regions, with corners at least `min_radius`.
float union_distance(vec2 p, float min_radius) {
    float d = 1.0e6;
    for (int i = 0; i < 8; i++) {
        if (float(i) >= shape_count) {
            break;
        }
        vec4 rect = shapes[i / 4][i - (i / 4) * 4];
        float r = radii[i / 4][i - (i / 4) * 4];
        // A tip: −1 points down, −2 up, −3 left, −4 right, turned into the downward frame (a
        // reflection or a transposition, which keep distances). A tip joins its body with a
        // small fillet (~2 pt, the concave sides of the reference), not with the merge distance
        // of separate shapes.
        bool tip = r < 0.0;
        vec2 near = p - rect.xy;
        vec2 far = rect.xy + rect.zw - p;
        float shape = r > -0.5 ? rounded_distance(p, rect, max(r, min_radius))
                    : r > -1.5 ? tip_distance(near, rect.zw)
                    : r > -2.5 ? tip_distance(vec2(near.x, far.y), rect.zw)
                    : r > -3.5 ? tip_distance(vec2(near.y, far.x), rect.wz)
                               : tip_distance(near.yx, rect.wz);
        d = smooth_min(d, shape, tip ? 2.0 * scale_px : merge);
    }
    return d;
}

// A measured edge profile at position s (points from the edge, 0 = the first one).
float profile_at(mat4 m, float s) {
    s = max(s, 0.0);
    float i0 = floor(s);
    float a = 0.0;
    float b = 0.0;
    for (int i = 0; i < 12; i++) {
        float v = m[i / 4][i - (i / 4) * 4];
        if (float(i) == i0) {
            a = v;
        }
        if (float(i) == i0 + 1.0) {
            b = v;
        }
    }
    return mix(a, b, s - i0);
}

// The dark outline just outside: alpha at position u (points from the edge, 0 = the first).
float outline_at(mat4 m, float u) {
    u = max(u, 0.0);
    return u < 1.0 ? mix(m[3][0], m[3][1], u) : mix(m[3][1], 0.0, clamp(u - 1.0, 0.0, 1.0));
}

// The source distance for a pixel at distance t, interpolating the curve.
float lens_source(mat4 curve, float t) {
    vec2 previous = vec2(-1.0, 0.0);
    for (int i = 0; i < 8; i++) {
        vec4 column = curve[i / 2];
        vec2 q = (i - (i / 2) * 2 == 0) ? column.xy : column.zw;
        if (q.x > 1.0e5) {
            break;
        }
        if (t <= q.x) {
            if (previous.x < 0.0) {
                return q.y;
            }
            return mix(previous.y, q.y, (t - previous.x) / max(q.x - previous.x, 1e-3));
        }
        previous = q;
    }
    return t;
}

void main() {
    vec2 p = v_coords * tex_size;
    float sd = union_distance(p, 0.0);
    float t = -sd;
    // Outward normal from the distance (central differences), on a shape with corners at
    // least as wide as the refraction band: without it, on the diagonal of a right angle the
    // normal jumps and the displacement leaves a seam.
    vec2 e = vec2(0.5, 0.0);
    vec2 n = vec2(union_distance(p + e.xy, lens_reach) - union_distance(p - e.xy, lens_reach),
                  union_distance(p + e.yx, lens_reach) - union_distance(p - e.yx, lens_reach));
    n = length(n) > 0.0 ? normalize(n) : vec2(0.0, -1.0);
    float vertical = n.y * n.y;

    // Lens: the pixel at distance t samples the background at distance s along the normal
    // (glass.md, "Refraction").
    float source = mix(lens_source(lens[0], t),
                       lens_source(n.y < 0.0 ? lens[1] : lens[2], t), vertical);
    vec2 sample_at = (p + n * (t - source)) / tex_size;

    vec3 background = texture2D(tex, sample_at).rgb;
    vec3 color = vec3(dot(row_r, background), dot(row_g, background), dot(row_b, background)) + offset;
    float luminance = (background.r + background.g + background.b) / 3.0;
    color += (curve2 + curve3 * luminance) * luminance * luminance;

    // Edge: the measured profiles, weighted by the orientation of the true shape's normal
    // (top, sides, bottom), so corners, tips and merged bridges blend continuously.
    vec2 m = vec2(union_distance(p + e.xy, 0.0) - union_distance(p - e.xy, 0.0),
                  union_distance(p + e.yx, 0.0) - union_distance(p - e.yx, 0.0));
    m = length(m) > 0.0 ? normalize(m) : vec2(0.0, -1.0);
    float w_top = max(-m.y, 0.0) * max(-m.y, 0.0);
    float w_bottom = max(m.y, 0.0) * max(m.y, 0.0);
    float w_side = m.x * m.x;
    float s_in = t / scale_px - 0.5;
    float offset_in = w_top * profile_at(edges[0], s_in) + w_side * profile_at(edges[1], s_in)
                    + w_bottom * profile_at(edges[2], s_in);
    if (offset_in > 0.0) {
        color += offset_in * min(vec3(1.0), (vec3(1.0) - color) / max(1.0 - fill_gray, 0.05));
    } else {
        color *= 1.0 + offset_in / max(fill_gray, 0.05);
    }
    float u_out = sd / scale_px - 0.5;
    float outline = w_top * outline_at(edges[0], u_out) + w_side * outline_at(edges[1], u_out)
                  + w_bottom * outline_at(edges[2], u_out);

    color = clamp(color, 0.0, 1.0);
    float coverage = clamp(0.5 - sd, 0.0, 1.0);
    vec4 result = (vec4(color, 1.0) * coverage + vec4(0.0, 0.0, 0.0, outline * (1.0 - coverage))) * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        result = vec4(0.0, 0.2, 0.0, 0.2) + result * 0.8;
#endif
    gl_FragColor = result;
}
