#version 300 es
precision highp float;
precision highp int;
uniform vec3 iResolution;
uniform float iTime;
uniform float uScale;
uniform float uSpeed;

// common.glsl inserted here

// 1D hash for generating pseudo-random per-layer values from variation seed
float hash1(float n) {
    return fract(sin(n) * 43758.5453123);
}

// Compute sine wave y-position for a dune layer.
// Includes a long-wavelength LFO that slowly modulates the wave shape.
float duneWave(float x, float baseY, float freq, float phase, float amplitude, float lfoFreq) {
    float lfo = sin(x * lfoFreq + phase * 0.7) * amplitude * 1.6;
    return baseY + sin(x * freq + phase) * amplitude + lfo;
}

out vec4 fragColor;

void main() {
    vec2 uv = distortUV(gl_FragCoord.xy / iResolution.xy);

    float x = uv.x * uScale * 10.0 + uSpeed;
    float y = uv.y;

    // Use variation as a seed to generate random per-layer parameters.
    // floor() so each integer step gives a new random set; fract part
    // is unused so the slider "clicks" between distinct configurations.
    float seed = floor(uVariation * 10.0) * 0.1;

    // Per-layer random frequencies (range ~1.0 to 3.0)
    float freq0 = 1.0 + hash1(seed + 0.1) * 2.0;
    float freq1 = 1.0 + hash1(seed + 0.2) * 2.0;
    float freq2 = 1.0 + hash1(seed + 0.3) * 2.0;

    // Per-layer random phases (range 0 to 2*PI)
    float p0 = hash1(seed + 0.4) * 6.2832;
    float p1 = hash1(seed + 0.5) * 6.2832;
    float p2 = hash1(seed + 0.6) * 6.2832;

    // Per-layer random LFO frequencies (~1/6th to 1/3rd of main freq)
    float lfo0 = freq0 * (0.15 + hash1(seed + 0.7) * 0.2);
    float lfo1 = freq1 * (0.15 + hash1(seed + 0.8) * 0.2);
    float lfo2 = freq2 * (0.15 + hash1(seed + 0.9) * 0.2);

    // 3 dune waves: color1 is background, colors 2-4 are the dune layers
    float w0 = duneWave(x, 0.30, freq0, p0, 0.07, lfo0);
    float w1 = duneWave(x, 0.55, freq1, p1, 0.08, lfo1);
    float w2 = duneWave(x, 0.80, freq2, p2, 0.07, lfo2);

    // Thin edge to preserve sine shape faithfully
    float edge = 0.005;

    // Silhouette masks: 1.0 when y < wave (pixel below wave on screen)
    float below0 = smoothstep(w0 + edge, w0 - edge, y);
    float below1 = smoothstep(w1 + edge, w1 - edge, y);
    float below2 = smoothstep(w2 + edge, w2 - edge, y);

    // Determine which band the pixel is in.
    // Color 1 (t<0.25) = background/sky above all waves
    // Color 2 (t~0.25-0.50) = top dune (between w2 and w1)
    // Color 3 (t~0.50-0.75) = middle dune (between w1 and w0)
    // Color 4 (t~0.75-1.0)  = bottom/foreground dune (below w0)
    float t;
    if (below2 < 0.5) {
        // Fade from background (color 1) to top dune (color 2) as y approaches w2 from above
        float localT = smoothstep(w2 + 0.30, w2, y);
        t = mix(0.0, 0.25, localT);
    } else if (below1 < 0.5) {
        // Between wave 2 (top) and wave 1: color 2
        float localT = (w2 - y) / max(w2 - w1, 0.001);
        localT = clamp(localT, 0.0, 1.0);
        t = mix(0.25, 0.50, localT);
    } else if (below0 < 0.5) {
        // Between wave 1 and wave 0: color 3
        float localT = (w1 - y) / max(w1 - w0, 0.001);
        localT = clamp(localT, 0.0, 1.0);
        t = mix(0.50, 0.75, localT);
    } else {
        // Below wave 0: foreground (color 4)
        float localT = smoothstep(w0, w0 - 0.30, y);
        t = mix(0.75, 1.0, localT);
    }

    vec3 color = paletteColor(t);
    color = applyLighting(color, t, uv);

    // Apply noise grain
    float n = hash(gl_FragCoord.xy);
    color += n * uNoise * 0.3;
    color = clamp(color, 0.0, 1.0);
    color = applyDither(color, gl_FragCoord.xy);
    fragColor = vec4(color, 1.0);
}
