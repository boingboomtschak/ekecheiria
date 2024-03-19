@group(0) @binding(0) 
var input_texture : texture_2d<f32>;

@group(0) @binding(1)
var output_texture : texture_storage_2d<rgba8unorm, write>;

const shift_radius : i32 = 5;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id : vec3<u32>) {
    let dimensions = vec2<i32>(textureDimensions(input_texture));
    let coords = vec2<i32>(global_id.xy);

    if(coords.x >= dimensions.x || coords.y >= dimensions.y) {
        return;
    }

    let color = textureLoad(input_texture, coords.xy, 0);
    let r = textureLoad(input_texture, vec2<i32>(coords.x - shift_radius, coords.y - shift_radius), 0).r;
    let g = color.g;
    let b = textureLoad(input_texture, vec2<i32>(coords.x + shift_radius, coords.y + shift_radius), 0   ).b;
    let shifted = vec4<f32>(r, g, b, color.a);

    textureStore(output_texture, coords.xy, shifted);
}