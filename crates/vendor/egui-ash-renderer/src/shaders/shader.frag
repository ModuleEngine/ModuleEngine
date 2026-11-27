#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec4 oColor;
layout(location = 1) in vec2 oUV;

layout(binding = 0, set = 0) uniform sampler2D fontsSampler;

layout(location = 0) out vec4 finalColor;

layout(constant_id = 0) const bool SRGB_FRAMEBUFFER = false;

const float GAMMA = 2.2;
const float INV_GAMMA = 1.0 / GAMMA;

vec3 LINEARtoSRGB(vec3 color) {
    return pow(color, vec3(INV_GAMMA));
}
vec4 LINEARtoSRGB(vec4 color) {
    return vec4(LINEARtoSRGB(color.rgb), color.a);
}
layout(push_constant) uniform FragmentMode {
    layout(offset = 64) uint graph_image;
} mode;

// The graph writes straight-alpha gamma-encoded UNORM, while ordinary egui
// textures already contain premultiplied sRGBA. Premultiply each source texel
// before bilinear interpolation, matching the existing Color32 conversion.
vec4 graphTexel(ivec2 p, ivec2 size) {
    uvec4 pixel = uvec4(round(255.0 * texelFetch(fontsSampler, clamp(p, ivec2(0), size - 1), 0)));
    uvec3 product = pixel.rgb * pixel.a + 128u;
    return vec4((product + (product >> 8u)) >> 8u, pixel.a) / 255.0;
}

void main() {
    if (mode.graph_image != 0u) {
        ivec2 size = textureSize(fontsSampler, 0);
        vec2 pos = oUV * vec2(size) - 0.5;
        ivec2 p = ivec2(floor(pos));
        vec2 f = fract(pos);
        vec4 sampled = mix(mix(graphTexel(p, size), graphTexel(p + ivec2(1,0), size), f.x),
                           mix(graphTexel(p + ivec2(0,1), size), graphTexel(p + 1, size), f.x), f.y);
        finalColor = LINEARtoSRGB(oColor) * sampled;
        return;
    }
    if (SRGB_FRAMEBUFFER) {
        finalColor = oColor * texture(fontsSampler, oUV);
    } else {
        finalColor = LINEARtoSRGB(oColor * texture(fontsSampler, oUV));
    }
}
