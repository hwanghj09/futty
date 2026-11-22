struct Light {
    position_range: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct Scene {
    view_projection: mat4x4<f32>,
    shadow_view_projection: mat4x4<f32>,
    sky_clip_to_world: mat4x4<f32>,
    sky_viewport: vec4<f32>,
    shadow_region: vec4<f32>,
    lighting: vec4<f32>,
    lights: array<Light, 8>,
}

@group(0) @binding(0) var<uniform> scene: Scene;
@group(1) @binding(0) var base_color: texture_2d<f32>;
@group(1) @binding(1) var base_sampler: sampler;
@group(2) @binding(0) var shadow_depth: texture_depth_2d;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) material: vec4<f32>,
    @location(3) world: vec3<f32>,
    @location(4) surface: vec4<f32>,
}

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) model_x: vec4<f32>,
    @location(4) model_y: vec4<f32>,
    @location(5) model_z: vec4<f32>,
    @location(6) model_w: vec4<f32>,
    @location(7) material: vec4<f32>,
    @location(8) surface: vec4<f32>,
) -> VertexOut {
    let world = mat4x4<f32>(model_x, model_y, model_z, model_w) * vec4<f32>(position, 1.0);
    let normal_matrix = mat3x3<f32>(
        cross(model_y.xyz, model_z.xyz),
        cross(model_z.xyz, model_x.xyz),
        cross(model_x.xyz, model_y.xyz),
    );
    return VertexOut(scene.view_projection * world, normal_matrix * normal, uv, material, world.xyz, surface);
}

fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055,
                  linear * 12.92, linear <= vec3<f32>(0.0031308));
}

fn background(position: vec2<f32>) -> vec3<f32> {
    if scene.sky_viewport.z == 0.0 {
        return vec3<f32>(0.065);
    }
    let ndc = position * scene.sky_viewport.xy + vec2<f32>(-1.0, 1.0);
    let ray = normalize((scene.sky_clip_to_world * vec4<f32>(ndc, 1.0, 1.0)).xyz);
    let horizon = vec3<f32>(0.68, 0.78, 0.88);
    let sky = mix(horizon, vec3<f32>(0.19, 0.42, 0.72), smoothstep(0.0, 0.8, ray.y));
    return mix(sky, vec3<f32>(0.20, 0.24, 0.29), smoothstep(0.0, 0.6, -ray.y));
}

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corners = array(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(corners[index], 0.0, 1.0);
}

@fragment
fn fs_sky(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(background(position.xy), 1.0);
}

fn shadow_visibility(world: vec3<f32>, lambert: f32) -> f32 {
    let range = scene.shadow_region.w;
    let distance = length(world - scene.shadow_region.xyz);
    if distance >= range {
        return 1.0;
    }
    let clip = scene.shadow_view_projection * vec4<f32>(world, 1.0);
    let uv = clip.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    if any(uv <= vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0)) || clip.z <= 0.0 || clip.z >= 1.0 {
        return 1.0;
    }
    let bias = 0.00007 + 0.00022 * (1.0 - lambert);
    let visible = textureSampleCompareLevel(shadow_depth, shadow_sampler, uv, clip.z - bias);
    return mix(visible, 1.0, smoothstep(0.85 * range, range, distance));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let sprite = in.material.a > 3.5;
    let texel_edge = vec2<f32>(0.5) / vec2<f32>(textureDimensions(base_color, 0));
    // Sprites reuse the surface parameters for their atlas rectangle.
    let extent = abs(in.surface.zw);
    let inset = min(texel_edge, extent * 0.5);
    let coords = select(in.uv, vec2<f32>(1.0) - in.uv, in.surface.zw < vec2<f32>(0.0));
    let sprite_uv = in.surface.xy + clamp(coords * extent, inset, extent - inset);
    let uv = select(in.uv, sprite_uv, sprite);
    let texel = textureSample(base_color, base_sampler, uv);
    if sprite {
        let color = linear_to_srgb(in.material.rgb * texel.rgb);
        return vec4<f32>(mix(color, vec3<f32>(0.66, 0.76, 0.87), select(0.0, 0.14, in.material.a > 4.5)), texel.a);
    }
    if in.material.a < 0.5 {
        let fade = smoothstep(8.0, 22.0, length(in.world.xz));
        return vec4<f32>(mix(in.material.rgb, background(in.position.xy), fade), 1.0);
    }
    let normal = in.normal * inverseSqrt(max(dot(in.normal, in.normal), 1e-30));
    var illumination = vec3<f32>(scene.lighting.x);
    var reflection = vec3<f32>(0.0);
    let view_offset = scene.shadow_region.xyz - in.world;
    let view_direction = view_offset / max(length(view_offset), 0.0001);
    let roughness = clamp(in.surface.a, 0.0, 1.0);
    for (var i = 0u; i < u32(scene.lighting.y); i++) {
        let light = scene.lights[i];
        var direction = light.position_range.xyz;
        var falloff = 1.0;
        if light.position_range.w > 0.0 {
            let offset = direction - in.world;
            let distance = length(offset);
            direction = offset / max(distance, 0.0001);
            falloff = max(1.0 - distance / light.position_range.w, 0.0);
            falloff *= falloff;
        }
        let lambert = max(dot(normal, direction), 0.0);
        if i + 1u == u32(scene.lighting.z) && lambert > 0.0 {
            falloff *= shadow_visibility(in.world, lambert);
        }
        illumination += light.color_intensity.rgb * (light.color_intensity.a * falloff * lambert);
        let halfway = direction + view_direction;
        let half_direction = halfway / max(length(halfway), 0.0001);
        let highlight = pow(max(dot(normal, half_direction), 0.0), mix(128.0, 2.0, roughness * roughness));
        reflection += light.color_intensity.rgb *
            (highlight * 0.25 * (1.0 - roughness) * (1.0 - roughness) * light.color_intensity.a * falloff * lambert);
    }
    let linear = in.material.rgb * texel.rgb * illumination + reflection + in.surface.rgb;
    let lit = linear_to_srgb(linear);
    let edge = min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y));
    let border = (1.0 - smoothstep(0.012, 0.027, edge)) * select(0.0, 1.0, in.material.a == 2.0);
    let selection = select(border, 0.09, in.material.a == 3.0);
    return vec4<f32>(mix(lit, vec3<f32>(0.66, 0.76, 0.87), selection), 1.0);
}
