// Internal compute ABI. Offsets into work are in bytes.
struct Config {
    float4 eyeNear, rightAspect, upFar, forwardFocal;
    uint4 counts;       // objects, draw runs, workgroups, use Hi-Z
    uint4 offsets;      // group counts/prefixes, commands, draw records, screen bounds
    uint4 viewport;     // width, height
};
[[vk::push_constant]] ConstantBuffer<Config> config;

struct Instance {
    float4 x, y, z;
    uint triangleBias, material, bounds, padding;
};
struct Bounds {
    float4 low, high;
};
[[vk::binding(0, 0)]] StructuredBuffer<Instance> instances;
[[vk::binding(1, 0)]] StructuredBuffer<uint4> draws;
[[vk::binding(2, 0)]] StructuredBuffer<Bounds> meshes;
[[vk::binding(3, 0)]] StructuredBuffer<uint4> runs;
[[vk::binding(4, 0)]] RWByteAddressBuffer work;
[[vk::binding(5, 0)]] StructuredBuffer<float> pyramid;

static const uint groupSize = 128;
static const uint finalCount = 0, seedCount = 4, frustumCount = 8;
static const uint flagsBegin = 12;

uint3 drawRecord(uint ordinal) {
    // Runs are ordered by logical ordinal. They reference physical slots
    // without having to upload the entire suffix of the scene after an edit.
    uint lo = 0, hi = config.counts.y;
    while (lo + 1 < hi) {
        uint middle = (lo + hi) / 2;
        if (runs[middle].x <= ordinal) {
            lo = middle;
        } else {
            hi = middle;
        }
    }
    uint3 run = runs[lo].xyz;
    return uint3(run.y + (ordinal - run.x), ordinal, run.z);
}

float3 cameraAligned(float3 coordinates, bool absoluteBasis) {
    float3 right = config.rightAspect.xyz;
    float3 up = config.upFar.xyz;
    float3 forward = config.forwardFocal.xyz;
    return float3(dot(coordinates, absoluteBasis ? abs(right) : right),
                  dot(coordinates, absoluteBasis ? abs(up) : up),
                  dot(coordinates, absoluteBasis ? abs(forward) : forward));
}

bool frustum(Instance instance, out float3 low, out float3 high) {
    Bounds bounds = meshes[instance.bounds];
    float3 center = bounds.low.xyz * 0.5 + bounds.high.xyz * 0.5;
    float3 halfSize = bounds.high.xyz * 0.5 - bounds.low.xyz * 0.5;
    float3 magnitude = max(abs(bounds.low.xyz), abs(bounds.high.xyz));
    float3 translation = float3(instance.x.w, instance.y.w, instance.z.w);
    float3 position = float3(dot(instance.x.xyz, center),
                            dot(instance.y.xyz, center),
                            dot(instance.z.xyz, center));
    float3 size = float3(dot(abs(instance.x.xyz), halfSize),
                        dot(abs(instance.y.xyz), halfSize),
                        dot(abs(instance.z.xyz), halfSize));
    // Inflate for float errors in the model, camera basis and clip equations.
    // Use the magnitudes before cancellation (important for a distant origin).
    float3 roundoff = float3(dot(abs(instance.x.xyz), magnitude),
                            dot(abs(instance.y.xyz), magnitude),
                            dot(abs(instance.z.xyz), magnitude));
    roundoff = (roundoff + abs(translation) + abs(config.eyeNear.xyz)) * 0.000008;
    float3 cameraCenter = cameraAligned(position + translation - config.eyeNear.xyz, false);
    float3 radius = cameraAligned(size + roundoff, true) + 0.000008;
    low = cameraCenter - radius;
    high = cameraCenter + radius;
    if (!all(isfinite(low)) || !all(isfinite(high))) {
        return true;
    }
    float2 scale = config.forwardFocal.w / float2(config.rightAspect.w, 1.0);
    return high.z >= config.eyeNear.w && low.z <= config.upFar.w
        && !any(max(low.xy, -high.xy) * scale > high.z);
}

float screenBounds(float3 low, float3 high, out uint4 pixels) {
    pixels = 0;
    if (low.z <= config.eyeNear.w || !all(isfinite(low)) || !all(isfinite(high))) {
        return 0.0; // Near-plane intersections and unbounded projections stay visible.
    }
    float2 scale = config.forwardFocal.w / float2(config.rightAspect.w, 1.0);
    float2 minXY = float2(low.x, -high.y) * scale;
    float2 maxXY = float2(high.x, -low.y) * scale;
    float2 lo = min(minXY / low.z, minXY / high.z);
    float2 hi = max(maxXY / low.z, maxXY / high.z);
    if (!all(isfinite(lo)) || !all(isfinite(hi))) {
        return 0.0;
    }
    float2 size = float2(config.viewport.xy);
    // Include the rasterization boundary and the pixels reached by rounding.
    pixels.xy = uint2(clamp((lo * 0.5 + 0.5) * size - 1.0, 0.0, size - 1.0));
    pixels.zw = uint2(clamp((hi * 0.5 + 0.5) * size + 1.0, 0.0, size - 1.0));
    float nearZ = config.eyeNear.w, farZ = config.upFar.w;
    return (low.z * farZ / (farZ - nearZ) - farZ * nearZ / (farZ - nearZ)) / low.z;
}

void storeDraw(uint index, uint indexCount, uint firstIndex, uint vertexOffset, uint firstInstance) {
    uint offset = config.offsets.y + index * 20;
    work.Store4(offset, uint4(indexCount, 1, firstIndex, vertexOffset));
    work.Store(offset + 16, firstInstance);
}

groupshared uint groupTotal;
void countGroup(uint lane, uint group, bool keep, bool countFrustum) {
    if (lane == 0) {
        groupTotal = 0;
    }
    GroupMemoryBarrierWithGroupSync();
    if (keep) {
        InterlockedAdd(groupTotal, 1);
    }
    GroupMemoryBarrierWithGroupSync();
    if (lane == 0) {
        if (countFrustum) {
            work.InterlockedAdd(frustumCount, groupTotal);
        } else {
            work.Store(config.offsets.x + group * 4, groupTotal);
        }
    }
}

[numthreads(128, 1, 1)]
void classify(uint3 thread : SV_DispatchThreadID, uint3 group : SV_GroupID, uint lane : SV_GroupIndex) {
    uint ordinal = thread.x;
    bool useHiZ = config.counts.w != 0;
    uint flag = 0;
    if (ordinal < config.counts.x) {
        uint slot = drawRecord(ordinal).x;
        uint4 draw = draws[slot];
        float3 low, high;
        if (draw.x != 0 && frustum(instances[slot], low, high)) {
            flag = 1;
            if (useHiZ) {
                uint4 rectangle;
                float nearest = screenBounds(low, high, rectangle);
                if (nearest > 0.0 && all(rectangle.zw - rectangle.xy >= 31)) {
                    // Only the minimum depth is used. The seed order need
                    // not be stable and its vertex stage can share vertices.
                    uint index;
                    work.InterlockedAdd(seedCount, 1, index);
                    storeDraw(index, draw.x, draw.z, 0, slot);
                    flag = 2;
                } else {
                    uint offset = config.offsets.w + ordinal * 20;
                    work.Store4(offset, rectangle);
                    work.Store(offset + 16, asuint(nearest));
                }
            }
        }
        work.Store(flagsBegin + ordinal * 4, flag);
    }
    countGroup(lane, group.x, flag != 0, useHiZ);
}

bool hidden(uint ordinal) {
    uint offset = config.offsets.w + ordinal * 20;
    float nearest = asfloat(work.Load(offset + 16));
    if (nearest <= 0.0) {
        return false;
    }
    uint4 rectangle = work.Load4(offset);
    uint2 span = rectangle.zw - rectangle.xy + 1;
    uint shift = uint(max(1, firstbithigh(max(span.x, span.y))));
    if (any((rectangle.zw >> shift) - (rectangle.xy >> shift) > 1)) {
        ++shift;
    }
    uint2 size = config.viewport.xy;
    uint levelStart = 0;
    for (uint level = 1; level < shift; ++level) {
        size = (size + 1) >> 1;
        levelStart += size.x * size.y;
    }
    size = (size + 1) >> 1;
    uint2 low = rectangle.xy >> shift;
    uint2 high = rectangle.zw >> shift;
    // Include every touched texel (at most 2x2), including uncovered samples
    // at depth 1. Use a strict comparison with a depth rounding allowance.
    float a = max(pyramid[levelStart + low.y * size.x + low.x],
                  pyramid[levelStart + low.y * size.x + high.x]);
    float b = max(pyramid[levelStart + high.y * size.x + low.x],
                  pyramid[levelStart + high.y * size.x + high.x]);
    return nearest > max(a, b) + 0.0000005;
}

[numthreads(128, 1, 1)]
void classifyOcclusion(uint3 thread : SV_DispatchThreadID, uint3 group : SV_GroupID,
                       uint lane : SV_GroupIndex) {
    uint ordinal = thread.x;
    bool keep = false;
    if (ordinal < config.counts.x) {
        uint flag = work.Load(flagsBegin + ordinal * 4);
        keep = flag == 2 || (flag == 1 && !hidden(ordinal));
        work.Store(flagsBegin + ordinal * 4, keep ? 1 : 0);
    }
    countGroup(lane, group.x, keep, false);
}

groupshared uint prefix[groupSize];
uint exclusivePrefix(uint lane, uint value) {
    // Workgroup-local Blelloch scan. It does not assume a subgroup size
    // or that neighboring local invocations belong to one subgroup.
    prefix[lane] = value;
    GroupMemoryBarrierWithGroupSync();
    [unroll] for (uint stride = 1; stride < groupSize; stride *= 2) {
        uint i = (lane + 1) * stride * 2 - 1;
        if (i < groupSize) {
            prefix[i] += prefix[i - stride];
        }
        GroupMemoryBarrierWithGroupSync();
    }
    if (lane == 0) {
        prefix[groupSize - 1] = 0;
    }
    GroupMemoryBarrierWithGroupSync();
    [unroll] for (uint stride = groupSize / 2; stride > 0; stride /= 2) {
        uint i = (lane + 1) * stride * 2 - 1;
        if (i < groupSize) {
            uint prior = prefix[i - stride];
            prefix[i - stride] = prefix[i];
            prefix[i] += prior;
        }
        GroupMemoryBarrierWithGroupSync();
    }
    return prefix[lane];
}

[numthreads(128, 1, 1)]
void scanGroups(uint lane : SV_GroupIndex) {
    uint count = config.counts.z;
    uint step = (count + groupSize - 1) / groupSize;
    uint first = lane * step, end = min(first + step, count);
    uint sum = 0;
    for (uint i = first; i < end; ++i) {
        sum += work.Load(config.offsets.x + 4 * i);
    }
    uint base = exclusivePrefix(lane, sum);
    for (uint i = first; i < end; ++i) {
        uint address = config.offsets.x + 4 * i;
        uint localCount = work.Load(address);
        work.Store(address, base);
        base += localCount;
    }
    if (lane == groupSize - 1) {
        work.Store(finalCount, base);
        if (config.counts.w == 0) {
            work.Store(frustumCount, base);
        }
    }
}

[numthreads(128, 1, 1)]
void compact(uint3 thread : SV_DispatchThreadID, uint3 group : SV_GroupID, uint lane : SV_GroupIndex) {
    uint ordinal = thread.x;
    bool keep = ordinal < config.counts.x && work.Load(flagsBegin + ordinal * 4) != 0;
    uint localIndex = exclusivePrefix(lane, keep ? 1 : 0);
    if (keep) {
        uint index = work.Load(config.offsets.x + group.x * 4) + localIndex;
        uint3 record = drawRecord(ordinal);
        uint4 draw = draws[record.x];
        // This draws consecutive triangle corners beginning at the mesh's
        // pooled index offset. The vertex stage follows them to shared vertices.
        storeDraw(index, draw.x, 0, draw.z, index);
        work.Store3(config.offsets.z + index * 12, record);
    }
}
