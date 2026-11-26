// Source-only S5 prototype. The fragment stage uses the stock Forward
// material lookup, lighting and output attachments.
struct Config {
    float4 eyeNear; float4 rightAspect; float4 upFar; float4 forwardFocal;
    float4 values[4];
};
[[vk::push_constant]] ConstantBuffer<Config> config;

static const uint meshletsPerTask = 32;
struct Payload { uint first; };
groupshared Payload batch;

[numthreads(32, 1, 1)]
void mainTask(uint3 group : SV_GroupID, uint lane : SV_GroupIndex) {
    uint first = group.x * meshletsPerTask;
    if (lane == 0) {
        batch.first = first;
    }
    DispatchMesh(min(meshletsPerTask, asuint(config.values[3].x) - first), 1, 1, batch);
}

struct Meshlet {
    uint firstVertex;
    uint firstPrimitive;
    uint vertexCount;
    uint primitiveCount;
    uint object;
};
struct Object {
    float4 x;
    float4 y;
    float4 z;
    float4 color;
};
[[vk::binding(1, 0)]] ByteAddressBuffer vertices;
[[vk::binding(2, 0)]] StructuredBuffer<Meshlet> meshlets;
[[vk::binding(3, 0)]] StructuredBuffer<uint> vertexIndices;
[[vk::binding(4, 0)]] StructuredBuffer<uint2> primitives;
[[vk::binding(5, 0)]] StructuredBuffer<Object> objects;

struct VertexOutput {
    float4 position : SV_Position;
    [[vk::location(1)]] float4 color : COLOR0;
    [[vk::location(2)]] float depth : TEXCOORD1;
    [[vk::location(4)]] float2 uv : TEXCOORD3;
};
struct PrimitiveOutput {
    [[vk::location(0)]] float3 normal : TEXCOORD0;
    [[vk::location(3)]] float2 ids : TEXCOORD2;
};

groupshared float3 world[64];

float3 worldPosition(uint vertex, Object object) {
    float3 p = asfloat(vertices.Load3(vertex * 20));
    precise float3 result = float3(object.x.x, object.y.x, object.z.x) * p.x;
    result += float3(object.x.y, object.y.y, object.z.y) * p.y;
    result += float3(object.x.z, object.y.z, object.z.z) * p.z;
    result += float3(object.x.w, object.y.w, object.z.w);
    return result;
}

VertexOutput cameraVertex(float3 worldPosition, float2 uv, float4 color) {
    float3 local = worldPosition - config.eyeNear.xyz;
    float3 p = float3(dot(local, config.rightAspect.xyz), dot(local, config.upFar.xyz),
                      dot(local, config.forwardFocal.xyz));
    float nearZ = config.eyeNear.w;
    float farZ = config.upFar.w;
    VertexOutput o;
    o.position = float4(p.x * config.forwardFocal.w / config.rightAspect.w,
                        -p.y * config.forwardFocal.w,
                        p.z * farZ / (farZ - nearZ) - farZ * nearZ / (farZ - nearZ), p.z);
    // Match the stock Forward color interpolant for each object.
    o.color = color;
    o.depth = p.z;
    o.uv = uv;
    return o;
}

[outputtopology("triangle")]
[numthreads(32, 1, 1)]
void mainMesh(uint3 group : SV_GroupID, uint lane : SV_GroupIndex, in payload Payload payload,
              out vertices VertexOutput vertexOut[64],
              out indices uint3 triangleOut[124],
              out primitives PrimitiveOutput primitiveOut[124]) {
    Meshlet m = meshlets[payload.first + group.x];
    Object object = objects[m.object];
    SetMeshOutputCounts(m.vertexCount, m.primitiveCount);

    for (uint i = lane; i < m.vertexCount; i += 32) {
        uint vertex = vertexIndices[m.firstVertex + i];
        float3 p = worldPosition(vertex, object);
        world[i] = p;
        vertexOut[i] = cameraVertex(p, asfloat(vertices.Load2(vertex * 20 + 12)), object.color);
    }
    GroupMemoryBarrierWithGroupSync();

    for (uint i = lane; i < m.primitiveCount; i += 32) {
        uint2 record = primitives[m.firstPrimitive + i];
        uint3 tri = (record.x >> uint3(0, 8, 16)) & 255;
        triangleOut[i] = tri;
        float3 a = world[tri.x];
        precise float3 face = cross(world[tri.y] - a, world[tri.z] - a);
        float length = sqrt(dot(face, face));
        PrimitiveOutput o;
        o.normal = length > 0.00001 ? face / length : float3(0, 1, 0);
        o.ids = float2(record.y, m.object + 1);
        primitiveOut[i] = o;
    }
}

// PerPrimitiveEXT (5271) on fragment inputs matches the mesh stage's
// primitive array. DXC accepts the SPIR-V decoration through this attribute.
struct PixelInput {
    [[vk::location(0), vk::ext_decorate(5271)]] float3 normal : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
    [[vk::location(2)]] float depth : TEXCOORD1;
    [[vk::location(3), vk::ext_decorate(5271)]] float2 ids : TEXCOORD2;
    [[vk::location(4)]] float2 uv : TEXCOORD3;
};
[[vk::binding(0, 1)]] Texture2D<float4> materialImages[];
[[vk::binding(1, 1)]] SamplerState materialSampler;
[[vk::binding(2, 1)]] StructuredBuffer<uint> materialIndices;

struct Targets { float4 color : SV_Target0; float4 normal : SV_Target1; float depth : SV_Target2; };
[[vk::ext_extension("SPV_EXT_mesh_shader")]]
[[vk::ext_capability(5283)]]
Targets main(PixelInput v) {
    uint row = asuint(config.values[3].w) + uint(v.ids.y) - 1;
    uint index = NonUniformResourceIndex(materialIndices[row]);
    float3 baseColor = v.color.rgb * materialImages[index].Sample(materialSampler, v.uv).rgb;
    float diffuse = 0.20 + 1.65 * abs(dot(normalize(v.normal), normalize(float3(-0.5, 1.0, 0.7))));
    Targets o;
    o.color = float4(baseColor * diffuse, v.ids.x);
    o.normal = float4(normalize(v.normal), v.ids.y);
    o.depth = v.depth;
    return o;
}
