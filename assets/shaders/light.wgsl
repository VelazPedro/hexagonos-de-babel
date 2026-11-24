// One additive triangle fan per light: the fan is the light's visibility
// polygon, so rasterising it produces the shadow mask.

struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    // Offset from the light centre divided by its radius.
    @location(2) uv: vec2<f32>,
    // Linear RGB times intensity.
    @location(4) color: vec4<f32>,
}

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
    let distance = length(in.uv);
    let reach = clamp(1.0 - distance, 0.0, 1.0);
    // Bright core with a soft shoulder, banded very slightly for a pixel feel.
    let smooth_falloff = reach * reach * (3.0 - 2.0 * reach);
    let banded = floor(smooth_falloff * 24.0 + 0.5) / 24.0;
    let falloff = mix(smooth_falloff, banded, 0.35);
    return vec4<f32>(in.color.rgb * falloff, 1.0);
}
