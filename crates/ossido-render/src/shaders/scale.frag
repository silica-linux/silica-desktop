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

// One axis of the Lanczos downscale from the 2x framebuffer to the display's native mode.
// `dir` is (1, 0) or (0, 1); the other axis keeps its size (the source and the output have
// the same length there). `lobes` is 2 or 3. The window of a native pixel is `lobes` native
// pixels to each side, so in source pixels it reaches lobes * ratio (ratio 1-2: at most 13
// taps). The weights are normalised and the edges repeat, as in the CPU reference
// (`scaler.rs`).
uniform vec2 dir;
uniform vec2 src_size;
uniform vec2 out_size;
uniform float lobes;

const float PI = 3.14159265358979;

float lanczos(float x) {
    x = abs(x);
    if (x < 1e-4) {
        return 1.0;
    }
    if (x >= lobes) {
        return 0.0;
    }
    float px = PI * x;
    return lobes * sin(px) * sin(px / lobes) / (px * px);
}

void main() {
    vec2 o = floor(v_coords * out_size);
    float ratio = dot(dir, src_size) / dot(dir, out_size);
    float centre = (dot(dir, o) + 0.5) * ratio - 0.5;
    float reach = lobes * ratio;
    float first = ceil(centre - reach);
    float last = dot(dir, src_size) - 1.0;
    vec4 sum = vec4(0.0);
    float total = 0.0;
    for (int k = 0; k < 14; k++) {
        float i = first + float(k);
        if (i > centre + reach) {
            break;
        }
        float w = lanczos((i - centre) / ratio);
        vec2 p = mix(o + vec2(0.5), vec2(clamp(i, 0.0, last) + 0.5), dir);
        sum += w * texture2D(tex, p / src_size);
        total += w;
    }
    vec4 color = clamp(sum / total, 0.0, 1.0);
    color.rgb = min(color.rgb, color.a);
    color *= alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
