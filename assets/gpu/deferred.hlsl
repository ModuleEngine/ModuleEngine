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
[[vk::binding(0,0)]] Texture2D<float4> albedo;
[[vk::binding(1,0)]] Texture2D<float4> normals;
[[vk::binding(2,0)]] Texture2D<float> depth;
#ifdef MODULE_ENGINE_DEFERRED_SURFACE
[[vk::binding(3,0)]] Texture2D<float4> surfacePayload;
[[vk::binding(4,0),vk::image_format("rgba32f")]] RWTexture2D<float4> outputColor;
#else
[[vk::binding(3,0),vk::image_format("rgba32f")]] RWTexture2D<float4> outputColor;
#endif
[numthreads(8,8,1)] void main(uint3 tid:SV_DispatchThreadID) {
    uint w,h;outputColor.GetDimensions(w,h);if(tid.x>=w||tid.y>=h)return;
    int2 p=int2(tid.xy);float4 a=albedo.Load(int3(p,0));float3 n=normals.Load(int3(p,0)).xyz;
    float d=depth.Load(int3(p,0));
#ifdef MODULE_ENGINE_DEFERRED_SURFACE
    float4 payload = surfacePayload.Load(int3(p, 0));
#ifdef MODULE_ENGINE_CLUSTERED_LIGHTS
    if (d > 0) {
#else
    if (d > 0 && payload.a > 0) {
#endif
        float2 ndc = (float2(p) + 0.5) / float2(w, h) * 2.0 - 1.0;
        float3 forward = forwardFocal.xyz + rightAspect.xyz * (ndc.x * rightAspect.w / forwardFocal.w)
                                         - upFar.xyz * (ndc.y / forwardFocal.w);
#ifdef MODULE_ENGINE_CLUSTERED_LIGHTS
        ShadedSurface surface = legacySurface(n);
        if (payload.a > 0) surface = decodeSurfacePayload(payload, n);
        outputColor[p] = float4(shadeClustered(a.rgb, surface, normalize(-forward), eyeNear.xyz + d * forward,
                                               tid.xy, d, eyeNear.w, upFar.w), a.a);
#else
        ShadedSurface surface = decodeSurfacePayload(payload, n);
        outputColor[p] = float4(shadeMetallicRoughness(a.rgb, surface, normalize(-forward)), a.a);
#endif
        return;
    }
#endif
    float diffuse=0.20+1.65*abs(dot(n,normalize(float3(-0.5,1.0,0.7))));
    float specular=0.18*pow(saturate(abs(n.z)),12.0);
    outputColor[p]=d>0?float4(a.rgb*diffuse+specular,a.a):a;
}

