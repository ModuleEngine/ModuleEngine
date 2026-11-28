// GPU/host contract: two uint4 header rows, four float4 rows per light.
// Directional lights precede local lights. Local indices map to cluster bits.
// The first light row has world XYZ and inverse squared range.
static const uint LIGHT_TILE = 64u;
static const uint LIGHT_Z_SLICES = 16u;
[[vk::binding(8, 0)]] StructuredBuffer<float4> lightData;
#ifdef MODULE_ENGINE_BUILD_LIGHT_GRID
[[vk::binding(9, 0)]] RWStructuredBuffer<uint> lightGrid;
#else
[[vk::binding(9, 0)]] StructuredBuffer<uint> lightGrid;
#endif

uint4 lightCounts() { return asuint(lightData[0]); } // tiles X/Y, local, directional
uint4 lightExtent() { return asuint(lightData[1]); } // width, height, words per cluster

uint lightGridOffset(uint3 cell, uint4 counts, uint words) {
    return ((cell.z * counts.y + cell.y) * counts.x + cell.x) * words;
}

uint lightRow(uint index) { return 2 + index * 4; }

#ifndef MODULE_ENGINE_BUILD_LIGHT_GRID
uint lightPixelOffset(uint2 pixel, float depth, float nearZ, float farZ) {
    uint4 counts = lightCounts();
    uint words = lightExtent().z;
    uint2 tile = min(pixel / LIGHT_TILE, counts.xy - 1);
    float slice = saturate(log2(max(depth, nearZ) / nearZ) / log2(farZ / nearZ));
    return lightGridOffset(uint3(tile, min(uint(slice * LIGHT_Z_SLICES), LIGHT_Z_SLICES - 1)), counts, words);
}
#endif
