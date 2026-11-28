struct Config {
    float4 eyeNear; float4 rightAspect; float4 upFar; float4 forwardFocal;
    float4 values[4];
};
[[vk::push_constant]] ConstantBuffer<Config> config;

groupshared uint masks[32];

[numthreads(32, 1, 1)]
void clusterLights(uint3 cell : SV_GroupID, uint lane : SV_GroupIndex) {
    uint4 counts = lightCounts();
    uint4 extent = lightExtent();
    float2 lo = float2(cell.xy * LIGHT_TILE) / float2(extent.xy) * 2.0 - 1.0;
    float2 hi = float2(min((cell.xy + 1u) * LIGHT_TILE, extent.xy)) / float2(extent.xy) * 2.0 - 1.0;
    float4 slopes = float4(lo.x, hi.x, -hi.y, -lo.y)
        * float4(config.rightAspect.ww, 1.0, 1.0) / config.forwardFocal.w;
    float4 radiusScale = sqrt(1.0 + slopes * slopes);
    float nearZ = config.eyeNear.w;
    float stepZ = log2(config.upFar.w / nearZ) / LIGHT_Z_SLICES;
    float2 depths = nearZ * exp2(float2(cell.z, cell.z + 1) * stepZ);

    masks[lane] = 0u;
    GroupMemoryBarrierWithGroupSync();
    for (uint index = lane; index < counts.z; index += 32u) {
        float4 light = lightData[lightRow(index + counts.w) + 3];
        float4 side = float4(light.x, -light.x, light.y, -light.y)
            + light.z * slopes * float4(-1.0, 1.0, -1.0, 1.0);
        if (light.z + light.w >= depths.x && light.z - light.w <= depths.y
            && all(side >= -light.w * radiusScale)) {
            InterlockedOr(masks[index >> 5u], 1u << (index & 31u));
        }
    }
    GroupMemoryBarrierWithGroupSync();
    if (lane < extent.z) {
        lightGrid[lightGridOffset(cell, counts, extent.z) + lane] = masks[lane];
    }
}
