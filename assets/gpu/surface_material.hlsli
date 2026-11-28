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

struct PunctualSurface {
    float3 normal;
    float3 view;
    float3 f0;
    float3 diffuseOverPi;
    float noV;
    float a2;
    float visibilityView;
};

PunctualSurface preparePunctualSurface(float3 base, ShadedSurface surface, float3 view) {
    PunctualSurface prepared;
    prepared.normal = surface.normal;
    prepared.view = view;
    prepared.f0 = lerp(0.04.xxx, base, surface.metallic);
    prepared.diffuseOverPi = base * ((1.0 - surface.metallic) / 3.14159265359);
    prepared.noV = max(0.0001, saturate(dot(surface.normal, view)));
    float roughness = max(surface.roughness, 0.045);
    float a = roughness * roughness;
    prepared.a2 = a * a;
    prepared.visibilityView = sqrt(prepared.noV * prepared.noV * (1.0 - prepared.a2) + prepared.a2);
    return prepared;
}

float3 punctualBrdf(PunctualSurface surface, float3 light, float NoL) {
    if (NoL <= 0.0) return 0.0.xxx;
    float3 sum = light + surface.view;
    float3 h = sum * rsqrt(max(dot(sum, sum), 1e-8));
    float NoH = saturate(dot(surface.normal, h));
    float VoH = saturate(dot(surface.view, h));
    float denominator = NoH * NoH * (surface.a2 - 1.0) + 1.0;
    float distribution = surface.a2 / (3.14159265359 * denominator * denominator);
    float gV = NoL * surface.visibilityView;
    float gL = surface.noV * sqrt(NoL * NoL * (1.0 - surface.a2) + surface.a2);
    float visibility = 0.5 / max(gV + gL, 0.0001);
    float complement = 1.0 - VoH;
    float squared = complement * complement;
    float3 fresnel = surface.f0 + (1.0 - surface.f0) * (squared * squared * complement);
    float3 diffuse = surface.diffuseOverPi * (1.0 - fresnel);
    return (diffuse + distribution * visibility * fresnel) * NoL;
}

float3 ambientSurface(float3 base, ShadedSurface surface) {
    float3 f0 = lerp(0.04.xxx, base, surface.metallic);
    return (base * (1.0 - surface.metallic) * 0.16 + f0 * 0.03) * surface.occlusion;
}

float3 shadeMetallicRoughness(float3 base, ShadedSurface surface, float3 view) {
    const float3 sun = normalize(float3(-0.5, 1.0, 0.7));
    PunctualSurface prepared = preparePunctualSurface(base, surface, view);
    return punctualBrdf(prepared, sun, saturate(dot(surface.normal, sun))) * 3.0
           + ambientSurface(base, surface) + surface.emissive;
}

#ifdef MODULE_ENGINE_CLUSTERED_LIGHTS
ShadedSurface legacySurface(float3 normal) {
    ShadedSurface surface;
    surface.normal = normal;
    surface.emissive = 0.0.xxx;
    surface.metallic = 0.0;
    surface.roughness = 1.0;
    surface.occlusion = 1.0;
    return surface;
}

float3 shadeClustered(float3 base, ShadedSurface surface, float3 view, float3 world, uint2 pixel,
                      float depth, float nearZ, float farZ) {
    PunctualSurface prepared = preparePunctualSurface(base, surface, view);
    uint4 counts = lightCounts();
    float3 direct = 0.0.xxx;
    for (uint id = 0; id < counts.w; ++id) {
        uint row = lightRow(id);
        float3 light = -lightData[row + 2].xyz;
        direct += punctualBrdf(prepared, light, saturate(dot(surface.normal, light))) * lightData[row + 1].rgb;
    }

    uint offset = lightPixelOffset(pixel, depth, nearZ, farZ);
    for (uint word = 0; word < lightExtent().z; ++word) {
        uint remaining = lightGrid[offset + word];
        while (remaining != 0u) {
            uint bit = firstbitlow(remaining);
            remaining &= remaining - 1u;
            uint row = lightRow(counts.w + word * 32u + bit);
            float4 source = lightData[row];
            float3 direction = source.xyz - world;
            float distanceSq = dot(direction, direction);
            float relativeDistanceSq = distanceSq * source.w;
            float unnormalizedNoL = dot(direction, surface.normal);
            if (distanceSq <= 1e-8 || relativeDistanceSq >= 1.0 || unnormalizedNoL <= 0.0) continue;
            float invDistance = rsqrt(distanceSq);
            float3 light = direction * invDistance;
            float NoL = saturate(unnormalizedNoL * invDistance);
            float4 energy = lightData[row + 1];
            float window = saturate(1.0 - relativeDistanceSq * relativeDistanceSq);
            float falloff = window * window / max(distanceSq, 0.01);
            if (energy.w >= 0.0) {
                float4 cone = lightData[row + 2];
                float angle = saturate((dot(-light, cone.xyz) - cone.w) / (energy.w - cone.w));
                falloff *= angle * angle;
            }
            direct += punctualBrdf(prepared, light, NoL) * (energy.rgb * falloff);
        }
    }
    return direct + ambientSurface(base, surface) + surface.emissive;
}
#endif
