"""生成应用图标：圆角渐变底 + 白色对话气泡 + 双向箭头。"""
from __future__ import annotations

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
SCALE = 4
SIZE = 512 * SCALE

STOPS = [
    (0.00, (67, 56, 202)),    # #4338CA
    (0.45, (109, 40, 217)),   # #6D28D9
    (1.00, (2, 132, 199)),    # #0284C7
]


def p(v: float) -> int:
    return int(round(v * SCALE))


def diagonal_gradient(size: int) -> Image.Image:
    y, x = np.mgrid[0:size, 0:size].astype(np.float32)
    t = (x / (size - 1) + y / (size - 1)) / 2.0
    out = np.zeros((size, size, 3), dtype=np.float32)
    for i in range(len(STOPS) - 1):
        p0, c0 = STOPS[i]
        p1, c1 = STOPS[i + 1]
        mask = (t >= p0) & (t <= p1)
        local = np.clip((t - p0) / (p1 - p0), 0, 1)
        for k in range(3):
            out[..., k] = np.where(mask, c0[k] + (c1[k] - c0[k]) * local, out[..., k])
    return Image.fromarray(out.astype(np.uint8), "RGB").convert("RGBA")


def rounded_mask(size: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=255)
    return mask


def _bezier(p0, p1, p2, steps: int = 28) -> list[tuple[float, float]]:
    points = []
    for i in range(steps + 1):
        t = i / steps
        x = (1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * p1[0] + t * t * p2[0]
        y = (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * p1[1] + t * t * p2[1]
        points.append((x, y))
    return points


def bubble_shape(draw_layer: Image.Image) -> None:
    draw_layer.rounded_rectangle(
        [p(80), p(96), p(448), p(352)], radius=p(56), fill=(255, 255, 255, 255)
    )
    left = _bezier((p(178), p(344)), (p(176), p(392)), (p(198), p(412)))
    right = _bezier((p(198), p(412)), (p(238), p(378)), (p(252), p(352)))
    tail = [(p(178), p(340))] + left + right + [(p(252), p(348))]
    draw_layer.polygon(tail, fill=(255, 255, 255, 255))


def draw_arrows(layer: Image.Image) -> None:
    d = ImageDraw.Draw(layer)
    w = p(20)
    for line in (
        [(p(186), p(196)), (p(322), p(196))],
        [(p(326), p(268)), (p(190), p(268))],
    ):
        d.line(line, fill=255, width=w, joint="curve")
        for pt in line:
            d.ellipse([pt[0] - w // 2, pt[1] - w // 2, pt[0] + w // 2, pt[1] + w // 2], fill=255)
    for chevron in (
        [(p(298), p(168)), (p(330), p(196)), (p(298), p(224))],
        [(p(214), p(240)), (p(182), p(268)), (p(214), p(296))],
    ):
        d.line(chevron, fill=255, width=w, joint="curve")
        for pt in (chevron[0], chevron[2]):
            d.ellipse([pt[0] - w // 2, pt[1] - w // 2, pt[0] + w // 2, pt[1] + w // 2], fill=255)


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    base = diagonal_gradient(SIZE)
    base.putalpha(rounded_mask(SIZE, p(116)))

    # 高光
    glow = Image.new("L", (SIZE, SIZE), 0)
    gd = ImageDraw.Draw(glow)
    gd.ellipse([p(-120), p(-180), p(380), p(320)], fill=46)
    glow = glow.filter(ImageFilter.GaussianBlur(p(60)))
    white = Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 255))
    base = Image.composite(white, base, glow.point(lambda v: v)).convert("RGBA")
    base.putalpha(rounded_mask(SIZE, p(116)))

    # 气泡阴影
    shadow = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    bubble_shape(ImageDraw.Draw(shadow))
    shadow = shadow.filter(ImageFilter.GaussianBlur(p(22)))
    shadow.putalpha(shadow.getchannel("A").point(lambda v: int(v * 0.38)))
    shadow = Image.alpha_composite(
        Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0)), shadow
    )
    offset = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    offset.paste(shadow, (0, p(16)))
    base = Image.alpha_composite(base, offset)

    # 气泡
    bubble = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    bubble_shape(ImageDraw.Draw(bubble))
    base = Image.alpha_composite(base, bubble)

    # 箭头（用渐变填充）
    mask = Image.new("L", (SIZE, SIZE), 0)
    draw_arrows(mask)
    arrows = diagonal_gradient(SIZE)
    arrows.putalpha(mask)
    base = Image.alpha_composite(base, arrows)

    # 输出
    final = base.resize((512, 512), Image.LANCZOS)
    final.save(ASSETS / "icon-512.png")
    final.resize((128, 128), Image.LANCZOS).save(ASSETS / "icon-128.png")
    final.resize((32, 32), Image.LANCZOS).save(ASSETS / "icon-32.png")

    icon = final.resize((256, 256), Image.LANCZOS)
    icon.save(
        ASSETS / "icon.ico",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print("saved:", ASSETS / "icon-512.png", ASSETS / "icon.ico")


if __name__ == "__main__":
    main()
