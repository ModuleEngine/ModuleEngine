// Stock scene shading. The Custom raster ABI still exposes only base color.
struct ShadedSurface {
    float3 normal;
    float3 emissive;
    float metallic;
    float roughness;
    float occlusion;
};

#ifndef MODULE_ENGINE_DEFERRED_SURFACE
struct SurfaceRow {
    uint4 maps;
    float4 factors;
    float4 emissiveCutoff;
    uint4 flags;
#ifndef MODULE_ENGINE_BINDLESS_MATERIALS
    float4 rects[4];
#endif
};

#ifdef MODULE_ENGINE_BINDLESS_MATERIALS
[[vk::binding(3, 1)]] StructuredBuffer<SurfaceRow> surfaceRows;
#elif defined(MODULE_ENGINE_TRANSPARENT_SURFACE)
[[vk::binding(3, 0)]] StructuredBuffer<SurfaceRow> surfaceRows;
#else
[[vk::binding(1, 0)]] StructuredBuffer<SurfaceRow> surfaceRows;
#endif

SurfaceRow readSurface(uint row) {
    uint count, stride;
    surfaceRows.GetDimensions(count, stride);
    return surfaceRows[row < count ? row : 0];
}

static const uint SURFACE_PBR = 1u;
static const uint SURFACE_DOUBLE_SIDED = 2u;
static const uint SURFACE_MASK = 4u;
static const uint SURFACE_BLEND = 8u;

float3 srgbToLinear(float3 s) {
    return lerp(s / 12.92, pow((s + 0.055) / 1.055, 2.4), step(0.04045, s));
}

float3 materialNormal(float3 n, float4 tangent, float3 pos, float2 uv,
                      float3 sampled, float scale, bool frontFace) {
    float3 t = tangent.xyz - n * dot(n, tangent.xyz);
    float3 b;
    float3 dx = ddx(pos), dy = ddy(pos);
    float2 u = ddx(uv), v = ddy(uv);
    float determinant = u.x * v.y - u.y * v.x;
    if (dot(t, t) > 1e-8) {
        t = normalize(t);
        b = cross(n, t) * (tangent.w < 0.0 ? -1.0 : 1.0);
    } else if (abs(determinant) > 1e-12) {
        t = (dx * v.y - dy * u.y) / determinant;
        b = (dy * u.x - dx * v.x) / determinant;
        t -= n * dot(n, t);
        if (dot(t, t) <= 1e-8) return frontFace ? n : -n;
        t = normalize(t);
        b = cross(n, t) * (dot(cross(n, t), b) < 0.0 ? -1.0 : 1.0);
    } else {
        return frontFace ? n : -n;
    }
    float2 xy = sampled.xy * 2.0 - 1.0;
    float z = sqrt(saturate(1.0 - dot(xy, xy)));
    float3 mapped = normalize((t * xy.x + b * xy.y) * scale + n * z);
    return frontFace ? mapped : -mapped;
}
#endif

float4 encodeSurfacePayload(ShadedSurface surface) {
    uint3 data = uint3(round(saturate(float3(surface.metallic, surface.roughness, surface.occlusion)) * 255.0));
    uint packed = data.x | (data.y << 8) | (data.z << 16);
    // The exact integer range 1..16777216 fits in the float attachment; zero is legacy.
    return float4(surface.emissive, float(packed + 1u));
}

ShadedSurface decodeSurfacePayload(float4 data, float3 normal) {
    uint packed = uint(data.a) - 1u;
    ShadedSurface surface;
    surface.normal = normal;
    surface.emissive = data.rgb;
    surface.metallic = float(packed & 255u) / 255.0;
    surface.roughness = float((packed >> 8) & 255u) / 255.0;
    surface.occlusion = float((packed >> 16) & 255u) / 255.0;
    return surface;
}

float3 shadeMetallicRoughness(float3 base, ShadedSurface surface, float3 view) {
    const float pi = 3.14159265359;
    const float3 light = normalize(float3(-0.5, 1.0, 0.7));
    float3 n = surface.normal;
    float3 h = normalize(light + view);
    float NoL = saturate(dot(n, light));
    float NoV = max(0.0001, saturate(dot(n, view)));
    float NoH = saturate(dot(n, h));
    float VoH = saturate(dot(view, h));
    float roughness = max(surface.roughness, 0.045);
    float a2 = roughness * roughness;
    a2 *= a2;
    float denominator = NoH * NoH * (a2 - 1.0) + 1.0;
    float distribution = a2 / (pi * denominator * denominator);
    float gV = NoL * sqrt(NoV * NoV * (1.0 - a2) + a2);
    float gL = NoV * sqrt(NoL * NoL * (1.0 - a2) + a2);
    float visibility = 0.5 / max(gV + gL, 0.0001);
    float3 f0 = lerp(0.04.xxx, base, surface.metallic);
    float3 fresnel = f0 + (1.0 - f0) * pow(1.0 - VoH, 5.0);
    float3 diffuseColor = base * (1.0 - surface.metallic);
    float3 diffuse = diffuseColor * (1.0 - fresnel) / pi;
    float3 direct = (diffuse + distribution * visibility * fresnel) * NoL * 3.0;
    float3 ambient = (diffuseColor * 0.16 + f0 * 0.03) * surface.occlusion;
    return direct + ambient + surface.emissive;
}
