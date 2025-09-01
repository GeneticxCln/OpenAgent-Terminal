#if defined(GLES2_RENDERER)
attribute vec2 aPos;

#else
layout (location = 0) in vec2 aPos;
#endif

void main() {
    gl_Position = vec4(aPos.xy, 0.0, 1.0);
}

