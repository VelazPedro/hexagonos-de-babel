// scene * (ambient + blurred light map) + emissive + bloom.

struct Settings {
    // rgb: ambient light, a: bloom strength.
    ambient: vec4<f32>,
    // rgb: flash colour, a: flash amount.
    flash: vec4<f32>,
    // x: texel width, y: texel height, z: vignette strength, w: time.
    params: vec4<f32>,
}

@group(2) @binding(0) var<uniform> settings: Settings;
@group(2) @binding(1) var scene_texture: texture_2d<f32>;
@group(2) @binding(2) var scene_sampler: sampler;
@group(2) @binding(3) var light_texture: texture_2d<f32>;
@group(2) @binding(4) var light_sampler: sampler;
@group(2) @binding(5) var emissive_texture: texture_2d<f32>;
@group(2) @binding(6) var emissive_sampler: sampler;

struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

// 9-tap tent blur: cheap soft shadow edges.
fn blurred_light(uv: vec2<f32>) -> vec3<f32> {
    let step = settings.params.xy * 2.0;
    var sum = textureSample(light_texture, light_sampler, uv).rgb * 4.0;
    sum += textureSample(light_texture, light_sampler, uv + vec2<f32>(step.x, 0.0)).rgb * 2.0;
    sum += textureSample(light_texture, light_sampler, uv - vec2<f32>(step.x, 0.0)).rgb * 2.0;
    sum += textureSample(light_texture, light_sampler, uv + vec2<f32>(0.0, step.y)).rgb * 2.0;
    sum += textureSample(light_texture, light_sampler, uv - vec2<f32>(0.0, step.y)).rgb * 2.0;
    sum += textureSample(light_texture, light_sampler, uv + step).rgb;
    sum += textureSample(light_texture, light_sampler, uv - step).rgb;
    sum += textureSample(light_texture, light_sampler, uv + vec2<f32>(step.x, -step.y)).rgb;
    sum += textureSample(light_texture, light_sampler, uv + vec2<f32>(-step.x, step.y)).rgb;
    return sum / 16.0;
}

fn glow_at(uv: vec2<f32>) -> vec3<f32> {
    let sample = textureSample(emissive_texture, emissive_sampler, uv);
    return sample.rgb * sample.a;
}

fn bloom(uv: vec2<f32>) -> vec3<f32> {
    let texel = settings.params.xy;
    var sum = vec3<f32>(0.0);
    var offsets = array<vec2<f32>, 12>(
        vec2<f32>(2.0, 0.0), vec2<f32>(-2.0, 0.0), vec2<f32>(0.0, 2.0), vec2<f32>(0.0, -2.0),
        vec2<f32>(3.0, 3.0), vec2<f32>(-3.0, 3.0), vec2<f32>(3.0, -3.0), vec2<f32>(-3.0, -3.0),
        vec2<f32>(5.0, 0.0), vec2<f32>(-5.0, 0.0), vec2<f32>(0.0, 5.0), vec2<f32>(0.0, -5.0),
    );
    for (var i = 0; i < 12; i++) {
        sum += glow_at(uv + offsets[i] * texel);
    }
    return sum / 12.0;
}

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_texture, scene_sampler, in.uv).rgb;
    let light = min(settings.ambient.rgb + blurred_light(in.uv), vec3<f32>(1.35));
    var color = scene * light;
    // Shadows lean toward a cool violet instead of black.
    let darkness = clamp(1.0 - max(light.r, max(light.g, light.b)), 0.0, 1.0);
    color += vec3<f32>(0.0045, 0.0022, 0.0125) * darkness;
    let emissive = textureSample(emissive_texture, emissive_sampler, in.uv);
    color = mix(color, emissive.rgb, emissive.a);
    color += bloom(in.uv) * settings.ambient.a;
    let centered = in.uv - vec2<f32>(0.5);
    let vignette = 1.0 - dot(centered, centered) * settings.params.z;
    color *= vignette;
    color = mix(color, settings.flash.rgb, settings.flash.a);
    return vec4<f32>(color, 1.0);
}
