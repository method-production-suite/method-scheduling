#!/usr/bin/env python3
"""Pictures for the Field Station sample's elements (M2c, decision 223): a few props and
vehicles drawn in code from plain shapes, not generated. Stdlib only, and the same bytes
every run.

Usage: python3 scripts/make-sample-pictures.py [OUT_DIR]   (default: fixtures/images)

Writes park-truck.png, quad-bike.png, hand-radio.png and paper-map.png, 240 by 160. The
sample (crates/method-schedule-core/src/sample.rs) stores them as images/<name>.png, beside
fixtures/field-station.mtsc, and the app carries the same files for the untitled sample.
"""

import os
import struct
import sys
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
W, H = 240, 160
SS = 4  # samples per pixel along each side

PAPER = (247, 246, 242)
INK = (28, 34, 40)


def hexc(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


class Canvas:
    def __init__(self, bg):
        self.px = [[bg] * W for _ in range(H)]

    def fill(self, inside, color, box):
        """Paints `color` where `inside(x, y)` holds, anti-aliased, within `box`."""
        x0, y0, x1, y1 = (max(0, int(box[0]) - 1), max(0, int(box[1]) - 1),
                          min(W, int(box[2]) + 2), min(H, int(box[3]) + 2))
        for y in range(y0, y1):
            row = self.px[y]
            for x in range(x0, x1):
                hit = 0
                for sy in range(SS):
                    for sx in range(SS):
                        if inside(x + (sx + 0.5) / SS, y + (sy + 0.5) / SS):
                            hit += 1
                if hit:
                    a = hit / (SS * SS)
                    r, g, b = row[x]
                    row[x] = (round(r + (color[0] - r) * a), round(g + (color[1] - g) * a),
                              round(b + (color[2] - b) * a))

    def rect(self, x0, y0, x1, y1, color, r=0):
        def inside(x, y):
            if not (x0 <= x <= x1 and y0 <= y <= y1):
                return False
            cx = min(max(x, x0 + r), x1 - r)
            cy = min(max(y, y0 + r), y1 - r)
            return (x - cx) ** 2 + (y - cy) ** 2 <= r * r
        self.fill(inside, color, (x0, y0, x1, y1))

    def circle(self, cx, cy, r, color):
        self.fill(lambda x, y: (x - cx) ** 2 + (y - cy) ** 2 <= r * r, color,
                  (cx - r, cy - r, cx + r, cy + r))

    def poly(self, pts, color):
        def inside(x, y):
            c = False
            j = len(pts) - 1
            for i in range(len(pts)):
                (xi, yi), (xj, yj) = pts[i], pts[j]
                if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
                    c = not c
                j = i
            return c
        xs = [p[0] for p in pts]
        ys = [p[1] for p in pts]
        self.fill(inside, color, (min(xs), min(ys), max(xs), max(ys)))

    def line(self, x0, y0, x1, y1, w, color):
        dx, dy = x1 - x0, y1 - y0
        n = (dx * dx + dy * dy) ** 0.5 or 1
        ox, oy = -dy / n * w / 2, dx / n * w / 2
        self.poly([(x0 + ox, y0 + oy), (x1 + ox, y1 + oy), (x1 - ox, y1 - oy), (x0 - ox, y0 - oy)], color)
        self.circle(x0, y0, w / 2, color)
        self.circle(x1, y1, w / 2, color)

    def png(self):
        rows = b"".join(b"\x00" + bytes(c for p in row for c in p) for row in self.px)

        def chunk(t, b):
            return struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xFFFFFFFF)
        ihdr = struct.pack(">IIBBBBB", W, H, 8, 2, 0, 0, 0)
        return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
                + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def ground(c, color=hexc("#d9d3c3")):
    c.rect(0, 128, W, H, color)


def wheel(c, x, y, r):
    c.circle(x, y, r, INK)
    c.circle(x, y, r * 0.55, hexc("#8a8f96"))
    c.circle(x, y, r * 0.2, INK)


def park_truck():
    c = Canvas(hexc("#e7eef5"))
    ground(c)
    green = hexc("#3f6b4a")
    c.rect(28, 70, 150, 118, green, 4)        # the bed
    c.poly([(150, 118), (150, 52), (186, 52), (210, 82), (214, 118)], green)  # the cab
    c.poly([(156, 58), (184, 58), (202, 82), (156, 82)], hexc("#bcd3e6"))     # its window
    c.rect(28, 64, 150, 72, hexc("#2f5238"), 2)  # the bed's rail
    c.rect(60, 84, 120, 98, hexc("#f2e8c9"), 2)  # the door badge
    c.rect(66, 89, 114, 93, green)
    c.rect(206, 98, 218, 104, hexc("#f6c453"), 2)  # the light
    c.rect(22, 112, 222, 120, INK, 3)            # the bumper line
    wheel(c, 64, 122, 15)
    wheel(c, 182, 122, 15)
    return c


def quad_bike():
    c = Canvas(hexc("#eef0e6"))
    ground(c, hexc("#cfc6ae"))
    red = hexc("#b5482f")
    wheel(c, 62, 112, 22)
    wheel(c, 178, 112, 22)
    c.poly([(40, 92), (90, 80), (150, 80), (204, 92), (196, 100), (44, 100)], red)  # the body
    c.rect(96, 66, 150, 82, INK, 6)             # the seat
    c.line(160, 82, 176, 56, 6, INK)            # the steering column
    c.line(164, 56, 192, 56, 6, INK)            # the bars
    c.rect(52, 84, 74, 90, hexc("#f6c453"), 2)  # a light
    c.rect(104, 92, 136, 98, hexc("#7d2f1f"), 2)
    return c


def hand_radio():
    c = Canvas(hexc("#f1ede4"))
    c.rect(150, 18, 158, 70, INK, 3)            # the aerial
    c.rect(96, 52, 164, 146, hexc("#2b3138"), 10)  # the body
    c.rect(106, 64, 154, 92, hexc("#a9c79a"), 4)   # the display
    c.rect(112, 72, 140, 76, hexc("#3f6b4a"))
    c.rect(112, 81, 132, 84, hexc("#3f6b4a"))
    for row in range(3):
        for col in range(4):
            c.circle(112 + col * 12, 106 + row * 12, 3.2, hexc("#8a8f96"))
    c.rect(86, 70, 96, 100, hexc("#b5482f"), 3)  # the talk button
    c.circle(116, 50, 5, hexc("#f6c453"))        # the knob
    return c


def paper_map():
    c = Canvas(hexc("#e9e4d6"))
    shades = [hexc("#f4ecd2"), hexc("#ece2c3"), hexc("#f4ecd2")]  # three folded panels
    c.poly([(30, 26), (90, 22), (90, 142), (30, 138)], shades[0])
    c.poly([(90, 22), (150, 26), (150, 138), (90, 142)], shades[1])
    c.poly([(150, 26), (210, 22), (210, 142), (150, 138)], shades[2])
    blue = hexc("#5b8fc9")
    c.poly([(36, 100), (70, 88), (96, 96), (120, 84), (150, 92), (176, 80), (204, 86),
            (204, 104), (172, 100), (146, 110), (118, 102), (92, 114), (64, 106), (36, 116)], blue)  # the lake
    for (x0, y0, x1, y1) in [(40, 50, 80, 40), (80, 40, 120, 56), (120, 56, 160, 44), (160, 44, 200, 58)]:
        c.line(x0, y0, x1, y1, 3, hexc("#b5482f"))  # the road
    c.circle(126, 62, 6, INK)                       # the station
    c.circle(126, 62, 3, hexc("#f6c453"))
    for x in (48, 62, 176, 190):
        c.poly([(x, 130), (x + 7, 116), (x + 14, 130)], hexc("#3f6b4a"))  # trees
    return c


PICTURES = {
    "park-truck.png": park_truck,
    "quad-bike.png": quad_bike,
    "hand-radio.png": hand_radio,
    "paper-map.png": paper_map,
}


def main(argv):
    out = argv[1] if len(argv) > 1 else os.path.join(ROOT, "fixtures", "images")
    os.makedirs(out, exist_ok=True)
    for name, draw in PICTURES.items():
        data = draw().png()
        path = os.path.join(out, name)
        old = open(path, "rb").read() if os.path.exists(path) else None
        if old != data:
            with open(path, "wb") as f:
                f.write(data)
        print(f"{name}: {len(data)} bytes{'' if old != data else ' (unchanged)'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
