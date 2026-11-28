// Shared ABI: 128-byte Vulkan push constants, camera in vectors 0..3,
// pass scalar parameters in values[0..3]. No hidden renderer-owned globals.
struct Config {
    float4 eyeNear; float4 rightAspect; float4 upFar; float4 forwardFocal;
    float4 values[4];
};
[[vk::push_constant]] ConstantBuffer<Config> config;
#define eyeNear config.eyeNear
#define rightAspect config.rightAspect
#define upFar config.upFar
#define forwardFocal config.forwardFocal
#define values config.values
struct Vertex {
    [[vk::location(0)]] float3 position : POSITION;
    [[vk::location(1)]] float3 normal : NORMAL;
    [[vk::location(2)]] float4 color : COLOR;
    [[vk::location(3)]] float2 ids : TEXCOORD0;
    [[vk::location(4)]] float2 uv : TEXCOORD1;
    [[vk::location(5)]] float4 atlasRect : TEXCOORD2;
    [[vk::location(6)]] float4 tangent : TANGENT;
};
struct SceneVarying {
    float4 position : SV_Position;
    [[vk::location(0)]] float3 normal : TEXCOORD0;
    [[vk::location(1)]] float4 color : COLOR0;
    [[vk::location(2)]] float depth : TEXCOORD1;
    [[vk::location(3)]] nointerpolation float2 ids : TEXCOORD2;
    [[vk::location(4)]] float2 uv : TEXCOORD3;
#ifdef MODULE_ENGINE_GPU_CULLING
    [[vk::location(5)]] nointerpolation uint materialRow : TEXCOORD4;
#else
    [[vk::location(5)]] nointerpolation float4 atlasRect : TEXCOORD4;
#endif
    [[vk::location(6)]] float3 world : TEXCOORD5;
    [[vk::location(7)]] float4 tangent : TEXCOORD6;
};
#ifndef MODULE_ENGINE_GPU_CULLING
SceneVarying mainVS(Vertex v) {
    float3 local = v.position - eyeNear.xyz;
    float3 p = float3(dot(local,rightAspect.xyz),dot(local,upFar.xyz),dot(local,forwardFocal.xyz));
    float nearZ=eyeNear.w, farZ=upFar.w;
    SceneVarying o;
    o.position=float4(p.x*forwardFocal.w/rightAspect.w,-p.y*forwardFocal.w,
                     p.z*farZ/(farZ-nearZ)-farZ*nearZ/(farZ-nearZ),p.z);
    o.normal=v.normal; o.color=v.color; o.depth=p.z; o.ids=v.ids; o.uv=v.uv; o.atlasRect=v.atlasRect;
    o.world=v.position; o.tangent=v.tangent; return o;
}
#endif
// Native geometry material contract: UV0, repeat wrapping, nearest base-color.
#ifdef MODULE_ENGINE_BINDLESS_MATERIALS
[[vk::binding(0,1)]] Texture2D<float4> materialImages[];
[[vk::binding(1,1)]] SamplerState materialSampler;
[[vk::binding(2,1)]] StructuredBuffer<uint> materialIndices;
uint materialRow(SceneVarying v) {
#ifdef MODULE_ENGINE_GPU_CULLING
    return v.materialRow;
#else
    return asuint(values[3].w) + (uint(v.ids.y) - 1);
#endif
}
float4 baseColor(SceneVarying v) {
    uint index = NonUniformResourceIndex(materialIndices[materialRow(v)]);
    return v.color * materialImages[index].Sample(materialSampler, v.uv);
}
#else
#ifdef MODULE_ENGINE_TRANSPARENT_SURFACE
[[vk::binding(2,0)]] Texture2D<float4> materialAtlas;
#else
[[vk::binding(0,0)]] Texture2D<float4> materialAtlas;
#endif
uint materialRow(SceneVarying v) { return uint(v.ids.y); }
float4 atlasSample(float4 rect, float2 uv) {
    int2 p=int2(rect.xy+min(floor(frac(uv)*rect.zw),rect.zw-1));
    return materialAtlas.Load(int3(p,0));
}
float4 baseColor(SceneVarying v) {
    float4 s=atlasSample(v.atlasRect,v.uv);
    float3 decoded=lerp(s.rgb/12.92,pow((s.rgb+0.055)/1.055,2.4),step(0.04045,s.rgb));
    return v.color*float4(decoded,s.a);
}
#endif

#ifdef MODULE_ENGINE_SURFACE_MATERIALS
float4 sampleMap(SurfaceRow row, uint slot, float2 uv) {
    if (row.maps[slot] == 0) return 1.0.xxxx;
#ifdef MODULE_ENGINE_BINDLESS_MATERIALS
    return materialImages[NonUniformResourceIndex(row.maps[slot])].Sample(materialSampler, uv);
#else
    float4 color = atlasSample(row.rects[slot], uv);
    return slot == 3 ? float4(srgbToLinear(color.rgb), color.a) : color;
#endif
}

ShadedSurface sampleSurface(SurfaceRow row, SceneVarying v, bool frontFace) {
    ShadedSurface surface;
    float3 normal = normalize(v.normal);
    surface.normal = frontFace ? normal : -normal;
    if (row.maps.y != 0) {
        surface.normal = materialNormal(normal, v.tangent, v.world, v.uv,
                                        sampleMap(row, 1, v.uv).xyz, row.factors.z, frontFace);
    }
    float3 metalRough = sampleMap(row, 0, v.uv).rgb;
    surface.metallic = row.factors.x * metalRough.b;
    surface.roughness = row.factors.y * metalRough.g;
    surface.occlusion = lerp(1.0, sampleMap(row, 2, v.uv).r, row.factors.w);
    surface.emissive = row.emissiveCutoff.rgb * sampleMap(row, 3, v.uv).rgb;
    return surface;
}
#endif

#ifdef MODULE_ENGINE_TRANSPARENT_SURFACE
[[vk::binding(1,0)]] Texture2D<float> opaqueDepth;
float4 main(SceneVarying v, bool frontFace:SV_IsFrontFace) : SV_Target0 {
    float d = opaqueDepth.Load(int3(int2(v.position.xy), 0));
    if (d > 0.0 && v.depth >= d) discard;
    SurfaceRow row = readSurface(materialRow(v));
    if ((row.flags.x & SURFACE_BLEND) == 0) discard;
    frontFace = frontFace != (abs(v.tangent.w) > 1.5);
    if (!frontFace && (row.flags.x & SURFACE_DOUBLE_SIDED) == 0) discard;
    float4 base = baseColor(v);
    if (base.a <= 0.0) discard;
    ShadedSurface surface = sampleSurface(row, v, frontFace);
    return float4(shadeMetallicRoughness(base.rgb, surface, normalize(eyeNear.xyz - v.world)), base.a);
}
#else
struct Targets {
    float4 color:SV_Target0;
    float4 normal:SV_Target1;
    float depth:SV_Target2;
#ifdef MODULE_ENGINE_GBUFFER_SURFACE
    float4 surface:SV_Target3;
#endif
};
Targets main(SceneVarying v, bool frontFace:SV_IsFrontFace) {
    float4 base = baseColor(v);
#ifdef MODULE_ENGINE_SURFACE_MATERIALS
    SurfaceRow row = readSurface(materialRow(v));
    if ((row.flags.x & SURFACE_PBR) != 0) {
        if ((row.flags.x & SURFACE_BLEND) != 0) discard;
        frontFace = frontFace != (abs(v.tangent.w) > 1.5);
        if (!frontFace && (row.flags.x & SURFACE_DOUBLE_SIDED) == 0) discard;
        if ((row.flags.x & SURFACE_MASK) != 0 && base.a < row.emissiveCutoff.a) discard;
        ShadedSurface surface = sampleSurface(row, v, frontFace);
        Targets o;
#ifdef MODULE_ENGINE_GBUFFER_SURFACE
        o.color = float4(base.rgb, v.ids.x);
        o.surface = encodeSurfacePayload(surface);
#else
        o.color = float4(shadeMetallicRoughness(base.rgb, surface, normalize(eyeNear.xyz - v.world)), v.ids.x);
#endif
        o.normal = float4(surface.normal, v.ids.y);
        o.depth = v.depth;
        return o;
    }
#endif
    Targets o;
#ifdef MODULE_ENGINE_GBUFFER_SURFACE
    o.color = float4(base.rgb, v.ids.x);
    o.surface = 0.0.xxxx;
#else
    float diffuse=0.20+1.65*abs(dot(normalize(v.normal),normalize(float3(-0.5,1.0,0.7))));
    o.color=float4(base.rgb*diffuse,v.ids.x);
#endif
    o.normal=float4(normalize(v.normal),v.ids.y);o.depth=v.depth;return o;
}
#endif

