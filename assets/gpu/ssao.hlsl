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
[[vk::binding(0,0)]] Texture2D<float4> color;
[[vk::binding(1,0)]] Texture2D<float4> normals;
[[vk::binding(2,0)]] Texture2D<float> depth;
[[vk::binding(3,0),vk::image_format("rgba32f")]] RWTexture2D<float4> outputColor;
// Reconstruct camera-relative world coordinates from linear camera Z.
// A raw Z difference alone mistakes an oblique surface for an occluder.
float3 surfacePosition(int2 p, float z, float2 size) {
    float2 ndc = (float2(p) + 0.5) / size * 2.0 - 1.0;
    return forwardFocal.xyz * z
        + rightAspect.xyz * (ndc.x * z * rightAspect.w / forwardFocal.w)
        - upFar.xyz * (ndc.y * z / forwardFocal.w);
}
[numthreads(8,8,1)] void main(uint3 tid:SV_DispatchThreadID) {
    uint w,h;outputColor.GetDimensions(w,h);if(tid.x>=w||tid.y>=h)return;
    int2 p=int2(tid.xy);float4 c=color.Load(int3(p,0));float d=depth.Load(int3(p,0));
    if(d<=0){outputColor[p]=c;return;}
    float3 n=normals.Load(int3(p,0)).xyz;float occluded=0;
    float3 center=surfacePosition(p,d,float2(w,h));
    int2 offsets[8]={int2(3,0),int2(-3,0),int2(0,3),int2(0,-3),int2(5,5),int2(-5,5),int2(5,-5),int2(-5,-5)};
    for(int i=0;i<8;++i){int2 q=clamp(p+offsets[i],int2(0,0),int2(w-1,h-1));
        float other=depth.Load(int3(q,0));
        if(other<=0)continue;
        float3 delta=surfacePosition(q,other,float2(w,h))-center;
        float distance=length(delta);
        // Only geometry above the receiving tangent plane can occlude it.
        // A world-distance falloff rejects unrelated foreground silhouettes.
        float height=dot(n,delta);
        occluded+=saturate((height-0.025)/max(distance,0.0001))
            *(1.0-smoothstep(0.0,1.4,distance));
    }
    float ao=1.0-saturate(values[0].x)*occluded/8.0;
    outputColor[p]=float4(c.rgb*ao,c.a);
}

