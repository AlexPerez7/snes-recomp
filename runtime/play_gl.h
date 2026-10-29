/* play_gl.h - the OpenGL path of the interactive frontend (option `shader = crt`): the picture goes through one
 * fragment shader that samples it sharp and adds a scanline beam, a phosphor mask, a curved tube and a glow. It
 * needs OpenGL 3.3 core; every function is loaded with SDL_GL_GetProcAddress (no GL header or library is linked), and
 * when anything is missing gl_setup() says so and play.c keeps drawing with the SDL renderer. Written from the
 * formulas, not taken from another project. play.c includes it. */
#ifndef PLAY_GL_H
#define PLAY_GL_H

#ifdef _WIN32
#define GLAPI_CALL APIENTRY
#else
#define GLAPI_CALL
#endif

typedef unsigned int GLenum_t;
typedef unsigned int GLuint_t;
typedef int GLint_t;
typedef int GLsizei_t;
typedef float GLfloat_t;
typedef char GLchar_t;

enum {
    GL_COLOR_BUFFER_BIT_ = 0x4000, GL_TEXTURE_2D_ = 0x0DE1, GL_RGB_ = 0x1907, GL_RGB8_ = 0x8051, GL_UNSIGNED_BYTE_ = 0x1401,
    GL_TEXTURE_MIN_FILTER_ = 0x2801, GL_TEXTURE_MAG_FILTER_ = 0x2800, GL_TEXTURE_WRAP_S_ = 0x2802, GL_TEXTURE_WRAP_T_ = 0x2803,
    GL_LINEAR_ = 0x2601, GL_CLAMP_TO_EDGE_ = 0x812F, GL_UNPACK_ALIGNMENT_ = 0x0CF5, GL_PACK_ALIGNMENT_ = 0x0D05,
    GL_TRIANGLE_STRIP_ = 0x0005, GL_VERTEX_SHADER_ = 0x8B31, GL_FRAGMENT_SHADER_ = 0x8B30, GL_COMPILE_STATUS_ = 0x8B81,
    GL_LINK_STATUS_ = 0x8B82, GL_FRAMEBUFFER_ = 0x8D40, GL_COLOR_ATTACHMENT0_ = 0x8CE0, GL_FRAMEBUFFER_COMPLETE_ = 0x8CD5,
    GL_VERSION_ = 0x1F02, GL_BLEND_ = 0x0BE2
};

/* the GL functions in use; `gl.name` is the pointer of glName */
static struct {
    void (GLAPI_CALL *ClearColor)(GLfloat_t, GLfloat_t, GLfloat_t, GLfloat_t);
    void (GLAPI_CALL *Clear)(GLuint_t);
    void (GLAPI_CALL *Viewport)(GLint_t, GLint_t, GLsizei_t, GLsizei_t);
    void (GLAPI_CALL *Disable)(GLenum_t);
    void (GLAPI_CALL *GenTextures)(GLsizei_t, GLuint_t *);
    void (GLAPI_CALL *DeleteTextures)(GLsizei_t, const GLuint_t *);
    void (GLAPI_CALL *BindTexture)(GLenum_t, GLuint_t);
    void (GLAPI_CALL *TexImage2D)(GLenum_t, GLint_t, GLint_t, GLsizei_t, GLsizei_t, GLint_t, GLenum_t, GLenum_t, const void *);
    void (GLAPI_CALL *TexSubImage2D)(GLenum_t, GLint_t, GLint_t, GLint_t, GLsizei_t, GLsizei_t, GLenum_t, GLenum_t, const void *);
    void (GLAPI_CALL *TexParameteri)(GLenum_t, GLenum_t, GLint_t);
    void (GLAPI_CALL *PixelStorei)(GLenum_t, GLint_t);
    void (GLAPI_CALL *DrawArrays)(GLenum_t, GLint_t, GLsizei_t);
    void (GLAPI_CALL *ReadPixels)(GLint_t, GLint_t, GLsizei_t, GLsizei_t, GLenum_t, GLenum_t, void *);
    const unsigned char *(GLAPI_CALL *GetString)(GLenum_t);
    GLuint_t (GLAPI_CALL *CreateShader)(GLenum_t);
    void (GLAPI_CALL *ShaderSource)(GLuint_t, GLsizei_t, const GLchar_t *const *, const GLint_t *);
    void (GLAPI_CALL *CompileShader)(GLuint_t);
    void (GLAPI_CALL *GetShaderiv)(GLuint_t, GLenum_t, GLint_t *);
    void (GLAPI_CALL *GetShaderInfoLog)(GLuint_t, GLsizei_t, GLsizei_t *, GLchar_t *);
    void (GLAPI_CALL *DeleteShader)(GLuint_t);
    GLuint_t (GLAPI_CALL *CreateProgram)(void);
    void (GLAPI_CALL *AttachShader)(GLuint_t, GLuint_t);
    void (GLAPI_CALL *LinkProgram)(GLuint_t);
    void (GLAPI_CALL *GetProgramiv)(GLuint_t, GLenum_t, GLint_t *);
    void (GLAPI_CALL *GetProgramInfoLog)(GLuint_t, GLsizei_t, GLsizei_t *, GLchar_t *);
    void (GLAPI_CALL *UseProgram)(GLuint_t);
    void (GLAPI_CALL *DeleteProgram)(GLuint_t);
    GLint_t (GLAPI_CALL *GetUniformLocation)(GLuint_t, const GLchar_t *);
    void (GLAPI_CALL *Uniform1i)(GLint_t, GLint_t);
    void (GLAPI_CALL *Uniform2f)(GLint_t, GLfloat_t, GLfloat_t);
    void (GLAPI_CALL *Uniform4f)(GLint_t, GLfloat_t, GLfloat_t, GLfloat_t, GLfloat_t);
    void (GLAPI_CALL *GenVertexArrays)(GLsizei_t, GLuint_t *);
    void (GLAPI_CALL *DeleteVertexArrays)(GLsizei_t, const GLuint_t *);
    void (GLAPI_CALL *BindVertexArray)(GLuint_t);
    void (GLAPI_CALL *GenFramebuffers)(GLsizei_t, GLuint_t *);
    void (GLAPI_CALL *DeleteFramebuffers)(GLsizei_t, const GLuint_t *);
    void (GLAPI_CALL *BindFramebuffer)(GLenum_t, GLuint_t);
    void (GLAPI_CALL *FramebufferTexture2D)(GLenum_t, GLenum_t, GLenum_t, GLuint_t, GLint_t);
    GLenum_t (GLAPI_CALL *CheckFramebufferStatus)(GLenum_t);
} gl;

/* The vertex shader draws a quad (a strip of four vertices from gl_VertexID) over the rectangle u_rect given as
 * left, bottom, right, top in normalised device coordinates. v_uv runs 0..1 across the rectangle, top to bottom. */
static const char *const gl_vs_src =
    "#version 330 core\n"
    "uniform vec4 u_rect;\n"
    "out vec2 v_uv;\n"
    "void main() {\n"
    "    vec2 p = vec2(float(gl_VertexID & 1), float((gl_VertexID >> 1) & 1));\n"
    "    v_uv = vec2(p.x, 1.0 - p.y);\n"
    "    gl_Position = vec4(mix(u_rect.xy, u_rect.zw, p), 0.0, 1.0);\n"
    "}\n";

/* The fragment shader, in the order it works:
 *  1. the position in the picture, bent by the curvature of the tube (what falls outside is black);
 *  2. sharp sampling: the fraction of the pixel is squeezed so that the inside of each picture pixel is flat and only
 *     a band one screen pixel wide around its edges is filtered; at a whole scale factor that is the same as nearest;
 *  3. glow: the mean of the four neighbours, squared, added;
 *  4. the beam: the distance to the middle of the picture line sets a bell, wider on bright pixels;
 *  5. the mask: a triad of screen columns, one per colour, with the brightness of the line kept.
 * Every effect leaves the colour alone at strength 0, and the colour only goes through a gamma curve when one is on. */
static const char *const gl_fs_src =
    "#version 330 core\n"
    "in vec2 v_uv;\n"
    "out vec4 o_color;\n"
    "uniform sampler2D u_tex;\n"
    "uniform vec4 u_src;\n"  /* x, y, width, height of the visible part of the picture, in picture pixels */
    "uniform vec2 u_tsz;\n"  /* size of the texture */
    "uniform vec2 u_out;\n"  /* size of the rectangle on the screen, in screen pixels */
    "uniform vec4 u_fx;\n"   /* beam, mask, curve, glow: 0..1 */
    "vec3 tap(vec2 p) { return texture(u_tex, p / u_tsz).rgb; }\n"
    "void main() {\n"
    "    vec2 uv = v_uv;\n"
    "    float edge = 1.0;\n"
    "    if (u_fx.z > 0.0) {\n"
    "        vec2 c = uv * 2.0 - 1.0;\n"
    "        float k = u_fx.z * 0.25;\n"
    "        c.x *= 1.0 + k * c.y * c.y;\n"
    "        c.y *= 1.0 + k * c.x * c.x;\n"
    "        uv = c * 0.5 + 0.5;\n"
    "        vec2 s = smoothstep(vec2(0.0), 1.5 / u_out, uv) * (1.0 - smoothstep(1.0 - 1.5 / u_out, vec2(1.0), uv));\n"
    "        edge = s.x * s.y;\n"
    "    }\n"
    "    vec2 p = u_src.xy + uv * u_src.zw;\n"
    "    vec2 scale = max(u_out / u_src.zw, vec2(1.0));\n"
    "    vec2 fl = floor(p), fr = p - fl;\n"
    "    vec2 h = 0.5 - 0.5 / scale, o = fr - 0.5;\n"
    "    vec2 q = fl + 0.5 + (o - clamp(o, -h, h)) * scale;\n"
    "    vec3 col = tap(q);\n"
    "    bool fx = u_fx.x > 0.0 || u_fx.y > 0.0 || u_fx.w > 0.0;\n"
    "    if (fx) col = pow(col, vec3(2.2));\n"
    "    if (u_fx.w > 0.0) {\n"
    "        vec3 g = (tap(q + vec2(1.0, 0.0)) + tap(q - vec2(1.0, 0.0)) + tap(q + vec2(0.0, 1.0)) + tap(q - vec2(0.0, 1.0))) * 0.25;\n"
    "        g = pow(g, vec3(2.2));\n"
    "        col += u_fx.w * 0.5 * g * g;\n"
    "    }\n"
    "    if (u_fx.x > 0.0) {\n"
    "        float d = fr.y - 0.5;\n"
    "        float lum = dot(col, vec3(0.299, 0.587, 0.114));\n"
    "        float w = mix(0.16, 0.42, clamp(lum, 0.0, 1.0));\n"
    "        float beam = exp(-d * d / (2.0 * w * w));\n"
    "        col *= mix(1.0, beam, u_fx.x) * (1.0 + 0.35 * u_fx.x);\n"
    "    }\n"
    "    if (u_fx.y > 0.0) {\n"
    "        int t = int(gl_FragCoord.x) % 3;\n"
    "        vec3 m = vec3(1.0 - u_fx.y);\n"
    "        m[t] = 1.0;\n"
    "        col *= m * (3.0 / (m.x + m.y + m.z));\n"
    "    }\n"
    "    if (fx) col = pow(clamp(col, 0.0, 1.0), vec3(1.0 / 2.2));\n"
    "    o_color = vec4(col * edge, 1.0);\n"
    "}\n";

static GLuint_t gl_prog, gl_vao, gl_pic_tex;
static GLint_t gl_u_rect, gl_u_src, gl_u_tsz, gl_u_out, gl_u_fx, gl_u_tex;
static int gl_pic_w;

/* Loads the functions of the current context; 0 if any is missing. */
static int gl_load(void) {
    int bad = 0;
#define GLF(name) do { *(void **)&gl.name = SDL_GL_GetProcAddress("gl" #name); if (!gl.name) { fprintf(stderr, "warning: shader: OpenGL has no gl%s\n", #name); bad = 1; } } while (0)
    GLF(ClearColor); GLF(Clear); GLF(Viewport); GLF(Disable); GLF(GenTextures); GLF(DeleteTextures); GLF(BindTexture);
    GLF(TexImage2D); GLF(TexSubImage2D); GLF(TexParameteri); GLF(PixelStorei); GLF(DrawArrays); GLF(ReadPixels); GLF(GetString);
    GLF(CreateShader); GLF(ShaderSource); GLF(CompileShader); GLF(GetShaderiv); GLF(GetShaderInfoLog); GLF(DeleteShader);
    GLF(CreateProgram); GLF(AttachShader); GLF(LinkProgram); GLF(GetProgramiv); GLF(GetProgramInfoLog); GLF(UseProgram); GLF(DeleteProgram);
    GLF(GetUniformLocation); GLF(Uniform1i); GLF(Uniform2f); GLF(Uniform4f);
    GLF(GenVertexArrays); GLF(DeleteVertexArrays); GLF(BindVertexArray);
    GLF(GenFramebuffers); GLF(DeleteFramebuffers); GLF(BindFramebuffer); GLF(FramebufferTexture2D); GLF(CheckFramebufferStatus);
#undef GLF
    return !bad;
}

static GLuint_t gl_compile(GLenum_t kind, const char *src) {
    GLuint_t s = gl.CreateShader(kind);
    GLint_t ok = 0;
    gl.ShaderSource(s, 1, &src, NULL);
    gl.CompileShader(s);
    gl.GetShaderiv(s, GL_COMPILE_STATUS_, &ok);
    if (!ok) {
        char log[512];
        log[0] = 0;
        gl.GetShaderInfoLog(s, sizeof log, NULL, log);
        fprintf(stderr, "warning: shader: cannot compile the %s shader: %s\n", kind == GL_VERTEX_SHADER_ ? "vertex" : "fragment", log);
        gl.DeleteShader(s);
        return 0;
    }
    return s;
}

/* Sets up the program, the (empty) vertex array and the texture of the picture in the current context; 0 on failure. */
static int gl_setup(void) {
    GLuint_t vs, fs;
    GLint_t ok = 0;
    if (!gl_load()) return 0;
    vs = gl_compile(GL_VERTEX_SHADER_, gl_vs_src);
    fs = vs ? gl_compile(GL_FRAGMENT_SHADER_, gl_fs_src) : 0;
    if (!vs || !fs) { if (vs) gl.DeleteShader(vs); return 0; }
    gl_prog = gl.CreateProgram();
    gl.AttachShader(gl_prog, vs);
    gl.AttachShader(gl_prog, fs);
    gl.LinkProgram(gl_prog);
    gl.DeleteShader(vs);
    gl.DeleteShader(fs);
    gl.GetProgramiv(gl_prog, GL_LINK_STATUS_, &ok);
    if (!ok) {
        char log[512];
        log[0] = 0;
        gl.GetProgramInfoLog(gl_prog, sizeof log, NULL, log);
        fprintf(stderr, "warning: shader: cannot link the program: %s\n", log);
        gl.DeleteProgram(gl_prog);
        gl_prog = 0;
        return 0;
    }
    gl_u_rect = gl.GetUniformLocation(gl_prog, "u_rect");
    gl_u_src = gl.GetUniformLocation(gl_prog, "u_src");
    gl_u_tsz = gl.GetUniformLocation(gl_prog, "u_tsz");
    gl_u_out = gl.GetUniformLocation(gl_prog, "u_out");
    gl_u_fx = gl.GetUniformLocation(gl_prog, "u_fx");
    gl_u_tex = gl.GetUniformLocation(gl_prog, "u_tex");
    gl.GenVertexArrays(1, &gl_vao);
    gl.GenTextures(1, &gl_pic_tex);
    gl_pic_w = 0;
    return 1;
}

/* Frees what gl_setup() made (the context goes with the window, destroyed by the caller). */
static void gl_teardown(void) {
    if (gl_prog) gl.DeleteProgram(gl_prog);
    if (gl_vao) gl.DeleteVertexArrays(1, &gl_vao);
    if (gl_pic_tex) gl.DeleteTextures(1, &gl_pic_tex);
    gl_prog = gl_vao = gl_pic_tex = 0;
}

/* Draws the picture `fb` (pw x SCREEN_H, RGB24) into the target of the current framebuffer, of aw x ah pixels, in the
 * rectangle (rx, ry, rw, rh) counted from the top left, showing the picture lines `top` to `top + lines - 1`. The
 * strengths are those of the options, 0 to 100. */
static void gl_draw(const uint8_t *fb, int pw, int ph, int aw, int ah, int rx, int ry, int rw, int rh, int top, int lines,
                    int beam, int mask, int curve, int glow) {
    gl.Viewport(0, 0, aw, ah);
    gl.ClearColor(0.0f, 0.0f, 0.0f, 1.0f);
    gl.Clear(GL_COLOR_BUFFER_BIT_);
    gl.Disable(GL_BLEND_);
    gl.BindTexture(GL_TEXTURE_2D_, gl_pic_tex);
    gl.PixelStorei(GL_UNPACK_ALIGNMENT_, 1);
    if (gl_pic_w != pw) {
        gl.TexParameteri(GL_TEXTURE_2D_, GL_TEXTURE_MIN_FILTER_, GL_LINEAR_);
        gl.TexParameteri(GL_TEXTURE_2D_, GL_TEXTURE_MAG_FILTER_, GL_LINEAR_);
        gl.TexParameteri(GL_TEXTURE_2D_, GL_TEXTURE_WRAP_S_, GL_CLAMP_TO_EDGE_);
        gl.TexParameteri(GL_TEXTURE_2D_, GL_TEXTURE_WRAP_T_, GL_CLAMP_TO_EDGE_);
        gl.TexImage2D(GL_TEXTURE_2D_, 0, GL_RGB8_, pw, ph, 0, GL_RGB_, GL_UNSIGNED_BYTE_, fb);
        gl_pic_w = pw;
    } else {
        gl.TexSubImage2D(GL_TEXTURE_2D_, 0, 0, 0, pw, ph, GL_RGB_, GL_UNSIGNED_BYTE_, fb);
    }
    gl.UseProgram(gl_prog);
    gl.Uniform1i(gl_u_tex, 0);
    gl.Uniform4f(gl_u_rect, 2.0f * rx / aw - 1.0f, 1.0f - 2.0f * (ry + rh) / ah, 2.0f * (rx + rw) / aw - 1.0f, 1.0f - 2.0f * ry / ah);
    gl.Uniform4f(gl_u_src, 0.0f, (float)top, (float)pw, (float)lines);
    gl.Uniform2f(gl_u_tsz, (float)pw, (float)ph);
    gl.Uniform2f(gl_u_out, (float)rw, (float)rh);
    gl.Uniform4f(gl_u_fx, beam / 100.0f, mask / 100.0f, curve / 100.0f, glow / 100.0f);
    gl.BindVertexArray(gl_vao);
    gl.DrawArrays(GL_TRIANGLE_STRIP_, 0, 4);
}

#endif
