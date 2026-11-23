// Internal vertex stage. Keep the outputs compatible with the stock fragment stages.
struct Config {
    float4 eyeNear; float4 rightAspect; float4 upFar; float4 forwardFocal;
    float4 values[4];
};
[[vk::push_constant]] ConstantBuffer<Config> config;

[[vk::binding(1, 0)]] ByteAddressBuffer vertices;
[[vk::binding(2, 0)]] StructuredBuffer<uint> indices;

struct Instance {
    float4 x;
    float4 y;
    float4 z;
    uint triangleBias;
    uint material;
    uint bounds;
    uint padding;
};
[[vk::binding(3, 0)]] StructuredBuffer<Instance> instances;
[[vk::binding(4, 0)]] StructuredBuffer<float4> colors;

struct SceneVarying {
    float4 position : SV_Position;
    [[vk::location(0)]] float3 normal : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
    [[vk::location(2)]] float depth : TEXCOORD1;
    [[vk::location(3)]] nointerpolation float2 ids : TEXCOORD2;
    [[vk::location(4)]] float2 uv : TEXCOORD3;
#ifdef MODULE_ENGINE_GPU_CULLING
    [[vk::location(5)]] nointerpolation uint materialRow : TEXCOORD4;
#endif
};

#ifdef MODULE_ENGINE_GPU_CULLING
[[vk::binding(5, 0)]] ByteAddressBuffer visible;
#endif

float3 worldPosition(uint vertex, Instance instance) {
    float3 p = asfloat(vertices.Load3(vertex * 20));
    // Keep the same operation order as the neutral model transform.
    precise float3 result = float3(instance.x.x, instance.y.x, instance.z.x) * p.x;
    result += float3(instance.x.y, instance.y.y, instance.z.y) * p.y;
    result += float3(instance.x.z, instance.y.z, instance.z.z) * p.z;
    result += float3(instance.x.w, instance.y.w, instance.z.w);
    return result;
}

float3 cameraPosition(float3 world) {
    float3 local = world - config.eyeNear.xyz;
    return float3(dot(local, config.rightAspect.xyz), dot(local, config.upFar.xyz),
                  dot(local, config.forwardFocal.xyz));
}

float4 clipPosition(float3 p) {
    float nearZ = config.eyeNear.w;
    float farZ = config.upFar.w;
    return float4(p.x * config.forwardFocal.w / config.rightAspect.w,
                  -p.y * config.forwardFocal.w,
                  p.z * farZ / (farZ - nearZ) - farZ * nearZ / (farZ - nearZ), p.z);
}

// The occluder prepass uses pooled mesh indices directly. It only needs
// position, so neighboring triangle corners can reuse the same vertex.
float4 depthVS(uint vertexId : SV_VertexID, uint instanceId : SV_InstanceID) : SV_Position {
    return clipPosition(cameraPosition(worldPosition(vertexId, instances[instanceId])));
}

SceneVarying mainVS(uint vertexId : SV_VertexID, uint instanceId : SV_InstanceID) {
#ifdef MODULE_ENGINE_GPU_CULLING
    uint3 record = visible.Load3(asuint(config.values[3].y) + instanceId * 12);
    uint slot = record.x;
    uint objectId = record.y + 1;
    uint triangleBias = record.z;
#else
    uint slot = instanceId;
    uint objectId = instanceId + 1 + asuint(config.values[3].y);
    uint triangleBias = asuint(config.values[3].z);
#endif
    Instance instance = instances[slot];
    uint corner = vertexId % 3;
    uint first = vertexId - corner;
    float3 a = worldPosition(indices[first], instance);
    float3 b = worldPosition(indices[first + 1], instance);
    float3 c = worldPosition(indices[first + 2], instance);
    precise float3 face = cross(b - a, c - a);
    float length = sqrt(dot(face, face));
    float3 p = cameraPosition(corner == 0 ? a : corner == 1 ? b : c);
    SceneVarying o;
    o.position = clipPosition(p);
    if (!all(isfinite(a)) || !all(isfinite(b)) || !all(isfinite(c))) {
        o.position = float4(0, 0, -1, 1);
    }
    o.normal = length > 0.00001 ? face / length : float3(0, 1, 0);
    o.color = colors[instance.material];
    o.depth = p.z;
    o.ids = float2(vertexId / 3 + instance.triangleBias + triangleBias, objectId);
    o.uv = asfloat(vertices.Load2(indices[vertexId] * 20 + 12));
#ifdef MODULE_ENGINE_GPU_CULLING
    o.materialRow = instance.material + asuint(config.values[3].w);
#endif
    return o;
}
