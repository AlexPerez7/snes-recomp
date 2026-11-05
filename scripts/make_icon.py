"""Draws the engine icon (original art, no game graphics) and writes assets/snes-port.ico and runtime/play_icon.h.

    python scripts/make_icon.py

Needs Pillow. The results are committed, so building the engine does not need Python.
"""
import pathlib
from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parent.parent
N = 1024  # drawn large and shrunk, so the edges are smooth


def lerp(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def draw(bg_top, bg_bottom, gem_light, gem_dark, gem_shine, ring=None, bg=True):
    img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    # background: rounded square with a vertical gradient
    grad = Image.new("RGBA", (N, N))
    gd = ImageDraw.Draw(grad)
    for y in range(N):
        gd.line([(0, y), (N, y)], fill=lerp(bg_top, bg_bottom, y / N) + (255,))
    mask = Image.new("L", (N, N), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, N - 1, N - 1], radius=int(N * 0.22), fill=255)
    if bg:
        img.paste(grad, (0, 0), mask)
    d = ImageDraw.Draw(img)
    if bg and ring:
        d.rounded_rectangle([N * 0.03, N * 0.03, N * 0.97, N * 0.97], radius=int(N * 0.19), outline=ring + (255,), width=int(N * 0.028))
    s = N / 256.0
    P = lambda x, y: (x * s, y * s)
    top, ur, lr, bot, ll, ul = P(128, 30), P(190, 88), P(190, 168), P(128, 226), P(66, 168), P(66, 88)
    mid_top, mid_bot = P(128, 88), P(128, 168)
    # a soft shadow below the gem
    sh = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    ImageDraw.Draw(sh).polygon([P(x, y + 8) for x, y in [(128, 30), (190, 88), (190, 168), (128, 226), (66, 168), (66, 88)]], fill=(0, 0, 0, 90))
    img = Image.alpha_composite(img, sh)
    d = ImageDraw.Draw(img)
    d.polygon([top, ur, lr, bot, ll, ul], fill=gem_dark + (255,))              # the whole gem, dark
    d.polygon([top, mid_top, mid_bot, bot, ll, ul], fill=gem_light + (255,))   # left half, light
    d.polygon([top, ur, mid_top], fill=lerp(gem_light, gem_shine, 0.6) + (255,))  # top facet, right
    d.polygon([top, mid_top, ul], fill=gem_shine + (255,))                     # top facet, left
    edge = lerp(gem_dark, (0, 0, 0), 0.35)
    d.line([top, ur, lr, bot, ll, ul, top], fill=edge + (255,), width=int(N * 0.012), joint="curve")
    d.line([mid_top, mid_bot], fill=edge + (255,), width=int(N * 0.008))
    return img


def write_ico(img, path):
    img.resize((256, 256), Image.LANCZOS).save(path, format="ICO", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (256, 256)])


def write_header(img, path, size=64):
    px = img.resize((size, size), Image.LANCZOS).convert("RGBA").load()
    rows = []
    for y in range(size):
        rows.append("    " + ",".join("0x%08X" % ((px[x, y][3] << 24) | (px[x, y][0] << 16) | (px[x, y][1] << 8) | px[x, y][2]) for x in range(size)) + ",")
    path.write_text(
        "/* The icon of the window (ARGB). Drawn by scripts/make_icon.py: original art. */\n"
        "#define PLAY_ICON_SIZE %d\n"
        "static const unsigned play_icon[PLAY_ICON_SIZE * PLAY_ICON_SIZE] = {\n%s\n};\n" % (size, "\n".join(rows)),
        encoding="utf-8",
    )



DENSITIES = {"mdpi": 1, "hdpi": 1.5, "xhdpi": 2, "xxhdpi": 3, "xxxhdpi": 4}


def write_android(res, app_name, colors, ring):
    """Escribe los recursos de Android: nombre, icono adaptable (fondo + gema + silueta monocroma) y el icono clásico."""
    res.mkdir(parents=True, exist_ok=True)
    (res / "values").mkdir(exist_ok=True)
    (res / "values" / "strings.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n    <string name="app_name">%s</string>\n</resources>\n' % app_name, encoding="utf-8")
    (res / "mipmap-anydpi-v26").mkdir(exist_ok=True)
    (res / "mipmap-anydpi-v26" / "ic_launcher.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>\n<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">\n'
        '    <background android:drawable="@mipmap/ic_launcher_background" />\n'
        '    <foreground android:drawable="@mipmap/ic_launcher_foreground" />\n'
        '    <monochrome android:drawable="@mipmap/ic_launcher_monochrome" />\n</adaptive-icon>\n', encoding="utf-8")
    top, bottom, light, dark, shine = colors
    full = draw(top, bottom, light, dark, shine, ring=ring)
    gem = draw(top, bottom, light, dark, shine, bg=False)
    # La capa adaptable es de 108 dp y solo se ve el círculo central de 66 dp: la gema se encoge para caber.
    layer = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    small = gem.resize((int(N * 0.72), int(N * 0.72)), Image.LANCZOS)
    layer.paste(small, ((N - small.width) // 2, (N - small.height) // 2), small)
    mono = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    mono.paste((255, 255, 255, 255), (0, 0), layer.getchannel("A").point(lambda a: 255 if a > 40 else 0))
    grad = Image.new("RGBA", (N, N))
    gd = ImageDraw.Draw(grad)
    for y in range(N):
        gd.line([(0, y), (N, y)], fill=lerp(top, bottom, y / N) + (255,))
    for name, scale in DENSITIES.items():
        d = res / ("mipmap-" + name)
        d.mkdir(exist_ok=True)
        px = round(48 * scale)
        full.resize((px, px), Image.LANCZOS).save(d / "ic_launcher.png")
        lp = round(108 * scale)
        layer.resize((lp, lp), Image.LANCZOS).save(d / "ic_launcher_foreground.png")
        mono.resize((lp, lp), Image.LANCZOS).save(d / "ic_launcher_monochrome.png")
        grad.resize((lp, lp), Image.LANCZOS).save(d / "ic_launcher_background.png")


if __name__ == "__main__":
    icon = draw((0x1C, 0x2F, 0x52), (0x0A, 0x12, 0x22), (0x4F, 0xD8, 0xE6), (0x1B, 0x8C, 0xA8), (0xB8, 0xF4, 0xFA))
    write_ico(icon, ROOT / "assets" / "snes-port.ico")
    write_header(icon, ROOT / "runtime" / "play_icon.h")
    write_android(ROOT / "android" / "app" / "src" / "main" / "res", "SNES Port",
                  ((0x1C, 0x2F, 0x52), (0x0A, 0x12, 0x22), (0x4F, 0xD8, 0xE6), (0x1B, 0x8C, 0xA8), (0xB8, 0xF4, 0xFA)), (0xF2, 0xC1, 0x4E))
    icon.resize((256, 256), Image.LANCZOS).save(pathlib.Path(__file__).with_name("icon_preview.png"))
    print("written")
