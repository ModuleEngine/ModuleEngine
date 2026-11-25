struct Screen {
    float2 points;
};
[[vk::push_constant]] ConstantBuffer<Screen> screen;

[[vk::binding(0, 0)]] Texture2D<float4> atlas;
[[vk::binding(1, 0)]] SamplerState atlas_sampler;

struct Vertex {
    [[vk::location(0)]] float2 logical_pos : POSITION;
    [[vk::location(1)]] float2 uv : TEXCOORD0;
    [[vk::location(2)]] float4 color : COLOR0;
};

struct Fragment {
    float4 position : SV_Position;
    [[vk::location(0)]] float2 uv : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
};

Fragment vertex_main(Vertex input) {
    Fragment output;
    output.position = float4(input.logical_pos * 2.0 / screen.points - 1.0, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

float4 fragment_main(Fragment input) : SV_Target0 {
    // Both operands contain premultiplied sRGBA bytes in UNORM formats.
    return input.color * atlas.Sample(atlas_sampler, input.uv);
}
