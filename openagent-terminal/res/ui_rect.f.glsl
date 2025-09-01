#if defined(GLES2_RENDERER)
#define float_t mediump float
#define vec2_t mediump vec2
#define vec4_t mediump vec4
#define FRAG_COLOR gl_FragColor

// Uniforms
uniform vec2_t uOrigin;      // top-left in pixels
uniform vec2_t uSize;        // size in pixels
uniform float_t uRadius;     // corner radius in pixels
uniform vec4_t uColor;       // rgba (alpha multiplies coverage)

// gl_FragCoord is in window coordinates (pixels)

#else
#define float_t float
#define vec2_t vec2
#define vec4_t vec4

out vec4 FragColor;
#define FRAG_COLOR FragColor

uniform vec2 uOrigin;      // top-left in pixels
uniform vec2 uSize;        // size in pixels
uniform float uRadius;     // corner radius in pixels
uniform vec4 uColor;       // rgba (alpha multiplies coverage)

#endif

// Signed distance to rounded rectangle centered at c with half-size b and radius r
float_t sdRoundedBox(vec2_t p, vec2_t b, float_t r) {
  vec2_t q = abs(p) - (b - vec2_t(r, r));
  return length(max(q, vec2_t(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
  // Convert to panel-local coordinates, centered
  vec2_t center = uOrigin + uSize * 0.5;
  vec2_t p = gl_FragCoord.xy - center;
  vec2_t halfSize = uSize * 0.5;

  float_t d = sdRoundedBox(p, halfSize, uRadius);
  float_t aa = fwidth(d);
  float_t alpha = smoothstep(0.0, -aa, d);

  FRAG_COLOR = vec4(uColor.rgb, uColor.a * alpha);
}

