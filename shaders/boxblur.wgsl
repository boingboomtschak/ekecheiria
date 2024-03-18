@group(0) @binding(0) 
var input_texture : texture_2d<f32>;

@group(0) @binding(1)
var output_texture : texture_storage_2d<rgba8unorm, write>;

const blur_radius : i32 = 3;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id : vec3<u32>) {
    let dimensions = vec2<i32>(textureDimensions(input_texture));
    let coords = vec2<i32>(global_id.xy);

    if(coords.x >= dimensions.x || coords.y >= dimensions.y) {
        return;
    }

    let blur_size = pow((f32(blur_radius) * 2.0 + 1.0), 2.0);
    let blur_denom = 1.0 / blur_size;

    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var i : i32 = coords.x - blur_radius; i <= coords.x + blur_radius; i = i + 1) {
        for (var j : i32 = coords.y - blur_radius; j <= coords.y + blur_radius; j = j + 1) {
            color = color + (blur_denom * textureLoad(input_texture, vec2<i32>(i, j), 0));
        }
    }

    textureStore(output_texture, coords.xy, color);
}