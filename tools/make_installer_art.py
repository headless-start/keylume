#!/usr/bin/env python3
"""Draw the Windows installer's pictures in Keylume's colours (needs Pillow):
the header strip (150x57) and the welcome/finish sidebar (164x314), as 24-bit BMPs in
src-tauri/windows/, with the app icon. Run it again after changing the brand colours or logo
(tools/make_icon.mjs first).

    python3 tools/make_installer_art.py
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

OUT = Path(__file__).resolve().parent.parent / "src-tauri" / "windows"
BG = (13, 15, 20)  # --bg-2, and MUI_BGCOLOR in installer-hooks.nsh
STOPS = [(31, 182, 255), (61, 107, 255), (106, 61, 255)]  # --grad
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"


def ramp(t: float) -> tuple[int, int, int]:
    """The brand gradient at 0..1."""
    t = max(0.0, min(1.0, t)) * (len(STOPS) - 1)
    i = min(int(t), len(STOPS) - 2)
    f = t - i
    a, b = STOPS[i], STOPS[i + 1]
    return tuple(round(a[k] + (b[k] - a[k]) * f) for k in range(3))


def logo(size: int) -> Image.Image:
    """The app icon (src-tauri/app-icon.png, drawn from src/assets/logo.svg), at `size`."""
    icon = Image.open(OUT.parent / "app-icon.png").convert("RGBA")
    return icon.resize((size, size), Image.LANCZOS)


def glow(canvas: Image.Image, shape: Image.Image, at: tuple[int, int], blur: float, strength: float) -> None:
    """Paste `shape` with a soft coloured glow behind it."""
    halo = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    halo.paste(shape, at, shape)
    halo = halo.filter(ImageFilter.GaussianBlur(blur))
    alpha = halo.getchannel("A").point(lambda a: int(a * strength))
    halo.putalpha(alpha)
    canvas.alpha_composite(halo)
    canvas.alpha_composite(shape, at)


def header() -> None:
    img = Image.new("RGBA", (150, 57), (*BG, 255))
    glow(img, logo(28), (12, 14), 6, 0.7)
    d = ImageDraw.Draw(img)
    d.text((48, 28), "Keylume", font=ImageFont.truetype(FONT, 17), fill=(238, 241, 247), anchor="lm")
    img.convert("RGB").save(OUT / "header.bmp")


def sidebar() -> None:
    w, h = 164, 314
    img = Image.new("RGBA", (w, h), (*BG, 255))
    # a lit keyboard, rows fading from cyan to violet, with its glow on the desk
    keys = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    k = ImageDraw.Draw(keys)
    cols, rows, unit, gap = 11, 5, 12, 2
    x0, y0 = (w - cols * unit) // 2, 170
    for r in range(rows):
        for c in range(cols):
            x, y = x0 + c * unit, y0 + r * unit
            k.rounded_rectangle((x, y, x + unit - gap, y + unit - gap), radius=2, fill=(*ramp(r / (rows - 1) * 0.85 + c / cols * 0.15), 255))
    case = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ImageDraw.Draw(case).rounded_rectangle((x0 - 6, y0 - 6, x0 + cols * unit + 4, y0 + rows * unit + 4), radius=6, fill=(24, 27, 36, 255))
    halo = keys.filter(ImageFilter.GaussianBlur(14))
    halo.putalpha(halo.getchannel("A").point(lambda a: int(a * 0.8)))
    img.alpha_composite(halo)
    img.alpha_composite(case)
    img.alpha_composite(keys)
    # the name, at the top
    mark = logo(40)
    glow(img, mark, ((w - 40) // 2, 54), 10, 0.8)
    d = ImageDraw.Draw(img)
    d.text((w / 2, 116), "Keylume", font=ImageFont.truetype(FONT, 20), fill=(238, 241, 247), anchor="mm")
    d.text((w / 2, 138), "Your keyboard, lit.", font=ImageFont.truetype(FONT.replace("-Bold", ""), 10), fill=(152, 160, 179), anchor="mm")
    img.convert("RGB").save(OUT / "sidebar.bmp")


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    header()
    sidebar()
    print("wrote", OUT / "header.bmp", "and", OUT / "sidebar.bmp")
