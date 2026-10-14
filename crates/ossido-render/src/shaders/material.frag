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
// Material color on the encoded sRGB values: out = M · in + b (rows of M).
uniform vec3 row_r;
uniform vec3 row_g;
uniform vec3 row_b;
uniform vec3 offset;
// Physical pixels per point.
uniform float scale_px;
// Edge lens: per side (0 sides, 1 top, 2 bottom) the (t, s) points in pixels: a pixel at
// distance t from the edge samples the background at distance s (negative: outside the edge).
// Columns of every mat4: t0, s0, t1, s1. lens_reach: how far the lens reaches.
uniform mat4 lens[3];
uniform float lens_reach;
// Edge: 1 pt luminous row on top and at the bottom (added per channel), inner glow (added on
// the first pixel and decaying in pixels), dark row on the sides (alpha of black).
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

// Downward tip in the rectangle: base on the top side, apex at the middle of the bottom side
// (distance from an isosceles triangle). The apex is rounded (~3.5 pt: in misc.md the tip is
// ~5 pt wide at 1 pt from the top). The triangle continues inside the body, twice as tall
// with the same sides: at the base the shape is thick enough that the body edge does not show
// through the tip.
float tip_distance(vec2 p, vec4 rect) {
    float round_px = 3.5 * scale_px;
    float apex = rect.y + rect.w - round_px;
    vec2 q = vec2(rect.z, 2.0 * (rect.w - round_px));
    vec2 local = vec2(abs(p.x - rect.x - rect.z * 0.5), apex - p.y);
    // Triangle with the apex at the origin, open upwards in `local` (iquilezles).
    vec2 a = local - q * clamp(dot(local, q) / dot(q, q), 0.0, 1.0);
    vec2 b = local - q * vec2(clamp(local.x / q.x, 0.0, 1.0), 1.0);
    float s = -sign(q.y);
    vec2 d = min(vec2(dot(a, a), s * (local.x * q.y - local.y * q.x)),
                 vec2(dot(b, b), s * (local.y - q.y)));
    return -sqrt(d.x) * sign(d.y) - round_px;
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
        // −1: downward tip; −2: upward (the same one mirrored on the horizontal axis).
        float shape = r < -1.5 ? tip_distance(vec2(p.x, 2.0 * rect.y + rect.w - p.y), rect)
                    : r < 0.0 ? tip_distance(p, rect)
                              : rounded_distance(p, rect, max(r, min_radius));
        d = smooth_min(d, shape, merge);
    }
    return d;
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

    // Edge: 1 light pt on top and at the bottom with the inner glow, 1 dark pt on the sides.
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
