@group(0) @binding(0)
var input_image: texture_storage_2d<bgra8unorm, read>;
@group(0) @binding(1)
var output_image: texture_storage_2d<bgra8unorm, write>;

//TODO: set to 8x8
@compute
@workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
   let size: vec2<u32> = textureDimensions(input_image);

   let load: vec4<f32> = textureLoad(input_image, global_id.xy);

   textureStore(output_image, size.xy - global_id.xy, vec4<f32>(load.r, load.g, load.b, 1.0));
}
