struct Config {
    uint2 sourceSize;
    uint2 targetSize;
    uint sourceOffset, targetOffset;
};
[[vk::push_constant]] ConstantBuffer<Config> config;
[[vk::binding(0, 0)]] Texture2D<float> depth;
[[vk::binding(1, 0)]] RWStructuredBuffer<float> pyramid;

float loadDepth(uint2 pixel, bool attachment) {
    if (attachment) {
        return depth.Load(int3(pixel, 0));
    }
    return pyramid[config.sourceOffset + pixel.y * config.sourceSize.x + pixel.x];
}

void reduce(uint2 pixel, bool attachment) {
    if (any(pixel >= config.targetSize)) {
        return;
    }
    uint2 lo = pixel * 2;
    uint2 hi = min(lo + 1, config.sourceSize - 1);
    float a = max(loadDepth(lo, attachment), loadDepth(uint2(hi.x, lo.y), attachment));
    float b = max(loadDepth(uint2(lo.x, hi.y), attachment), loadDepth(hi, attachment));
    pyramid[config.targetOffset + pixel.y * config.targetSize.x + pixel.x] = max(a, b);
}

[numthreads(8, 8, 1)]
void reduceDepth(uint3 thread : SV_DispatchThreadID) {
    reduce(thread.xy, true);
}

[numthreads(8, 8, 1)]
void reduceMip(uint3 thread : SV_DispatchThreadID) {
    reduce(thread.xy, false);
}
