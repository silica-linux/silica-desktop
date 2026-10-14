#!/usr/bin/env python3
"""Glass parity: renders our glass over the reference wallpapers, at the same places as the
reference captures, and measures the difference.

Usage: glass_parity.py [--reference DIR] [--out DIR] [--only SURFACE] [--skip-render] [--tips]

- reference: the silica-reference checkout (default: plan/reference of the main checkout).
- out: where renders, side-by-side images and `summary.json` go.

Metrics, per surface and appearance (all in 0–255 levels or pixels at 1×):
- fill: RMSE inside the shape over the gray and saw wallpapers (color transfer);
- blur: RMSE inside over the stripe wallpapers;
- refraction: RMSE of the apparent source displacement (px) near the edges, from the
  disp-x / disp-y wallpapers (level = coordinate mod 256);
- rim: RMSE of the luminance profile across the top, bottom and side edges on gray;
- silhouette: mean / max distance (px) between our edge and the reference edge, per row and
  per column, on gray.

With --tips, the tip outlines instead (TIP-01): our tips rendered over black, their outline
taken like the reference's (coverage on the luminance, the 0.5 crossing per column, depth from
the body edge as a function of the distance from the axis), compared with the reference
captures on black and with the measured model (fillet, flank, round apex). Side-by-side crops
(tip pointing down, 4x, reference left) go to `tips/`.
"""
import argparse
import json
import os
import subprocess
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
GRAY = np.array([0x7A, 0x7B, 0x80], float)

# A region: x, y, w, h (screen points), radius, material, tip ("none", "down", "up").
CC_REGIONS = [
    (2254, 70, 140, 64, 32, "regular", "none"),
    (2254, 146, 140, 64, 32, "regular", "none"),
    (2406, 70, 140, 140, 35, "regular", "none"),
    (2254, 222, 140, 64, 32, "regular", "none"),
    (2406, 222, 64, 64, 32, "regular", "none"),
    (2482, 222, 64, 64, 32, "regular", "none"),
    (2254, 298, 64, 64, 32, "regular", "none"),
    (2330, 298, 64, 64, 32, "regular", "none"),
    (2406, 298, 140, 64, 32, "regular", "none"),
    (2254, 374, 292, 64, 27.7, "regular", "none"),
    (2254, 450, 292, 64, 27.7, "regular", "none"),
]

SURFACES = {
    # Dock background (dock.md: x 816–1743, y 1356–1434, r ~22, circular arc).
    "dock": {
        "crop": (760, 1250, 1040, 190),
        "regions": [(816, 1356, 928, 79, 22, "clear", "none")],
    },
    # Control Center modules (controlcenter.md, with the privacy pill: Wi-Fi at y 70).
    "cc": {
        "crop": (2080, 0, 480, 560),
        "regions": CC_REGIONS,
        # A 64 × 64 tile: nothing near its edges (Wi-Fi has its disc).
        "rim_region": 5,
        "mask_appearance": "dark",
        # Some captures have a notification above the modules (see `shifted`).
        "check_shift": True,
    },
    # The File menu of the menu bar (menubar.md: body x 99–385, y 31–659, r ~12).
    "menu": {
        "crop": (0, 0, 800, 700),
        "regions": [(99, 31, 287, 629, 12, "thick", "none")],
        # The top samples the menu bar gradient, the left side the widgets.
        "rim_sides": ("bottom", "right"),
        # The desktop widgets are behind the left part (x < 190) and the menu bar gradient
        # behind the top (y < 120): measure where the glass sees only the wallpaper.
        "valid": (240, 140, 800, 700),
    },
    # The context menu of a Dock icon with its tip (misc.md: body x 829–1056, y 841–1337;
    # tip centered at x 856; TIP-01: 30.6 pt from fillet to fillet, 10.2 deep, sent as 30 × 10).
    "dockmenu": {
        "crop": (780, 800, 340, 600),
        "regions": [
            (829, 841, 228, 497, 12, "thick", "none"),
            (841, 1338, 30, 10, 0, "thick", "down"),
        ],
        # A widget sits above the menu.
        "rim_sides": ("bottom", "left", "right"),
        # In the dark capture the menu has another size: not comparable.
        "appearances": ("light",),
        "fullscreen": True,
    },
}

WALLPAPER = {
    "gray": "gray",
    "stripes-v": "stripes-v",
    "stripes-h": "stripes-h",
    "stripes-v-fine": "stripes-v-fine",
    "stripes-v-mid": "stripes-v-mid",
    "saw-gray": "saw-gray",
    "saw-r": "saw-r",
    "saw-g": "saw-g",
    "saw-b": "saw-b",
    "disp-x": "disp-x",
    "disp-y": "disp-y",
}


def load(path):
    """An image as float RGB in sRGB: reference captures carry the display profile (saturated
    colors differ from sRGB, glass.md › Colors) and are converted with it."""
    image = Image.open(path)
    icc = image.info.get("icc_profile")
    image = image.convert("RGB")
    if icc:
        from io import BytesIO
        from PIL import ImageCms

        try:
            source = ImageCms.ImageCmsProfile(BytesIO(icc))
            image = ImageCms.profileToProfile(image, source, ImageCms.createProfile("sRGB"))
        except (ImageCms.PyCMSError, OSError):
            pass
    return np.asarray(image).astype(float)


def load_files(ref):
    """Adds to every surface the capture files it is compared with. The table lives in the
    private reference checkout (`tools/glass_parity_files.json`): per surface, `files` maps a
    wallpaper to a path pattern with `{ap}`, and the optional `gray_light` is a full-screen
    capture used for the light gray case."""
    path = os.path.join(ref, "tools", "glass_parity_files.json")
    if not os.path.exists(path):
        sys.exit(f"capture table not found: {path} (it comes with silica-reference)")
    with open(path) as f:
        table = json.load(f)
    for surface, spec in SURFACES.items():
        if surface not in table:
            sys.exit(f"{path}: no entry for the surface {surface!r}")
        spec["files"] = table[surface]["files"]
        if "gray_light" in table[surface]:
            spec["gray_light"] = table[surface]["gray_light"]


def reference_file(ref, surface, wp, ap):
    spec = SURFACES[surface]
    if wp == "gray" and ap == "light" and "gray_light" in spec:
        return os.path.join(ref, spec["gray_light"]), True
    path = os.path.join(ref, spec["files"][wp].format(ap=ap))
    return path, spec.get("fullscreen", False)


def reference_crop(ref, surface, wp, ap):
    path, fullscreen = reference_file(ref, surface, wp, ap)
    if not os.path.exists(path):
        return None
    image = load(path)
    x, y, w, h = SURFACES[surface]["crop"]
    if fullscreen:
        image = image[y : y + h, x : x + w]
    return image


def our_crop(path, surface):
    image = load(path)
    x, y, w, h = SURFACES[surface]["crop"]
    return image[y : y + h, x : x + w]


def wallpaper_crop(ref, surface, wp):
    image = load(os.path.join(ref, "wallpapers", WALLPAPER[wp] + ".png"))
    x, y, w, h = SURFACES[surface]["crop"]
    return image[y : y + h, x : x + w]


def cases(ref, out, only):
    result = []
    for surface, spec in SURFACES.items():
        if only and surface not in only:
            continue
        for ap in spec.get("appearances", ("light", "dark")):
            for wp in spec["files"]:
                path, _ = reference_file(ref, surface, wp, ap)
                if not os.path.exists(path):
                    continue
                result.append(
                    {
                        "name": f"{surface}-{wp}-{ap}",
                        "dark": ap == "dark",
                        "wallpaper": os.path.join(ref, "wallpapers", WALLPAPER[wp] + ".png"),
                        "out": os.path.join(out, "ours", f"{surface}-{wp}-{ap}.png"),
                        "regions": [
                            {"x": x, "y": y, "w": w, "h": h, "r": r, "material": m, "tip": t}
                            for (x, y, w, h, r, m, t) in spec["regions"]
                        ],
                    }
                )
    return result


def render(spec_cases, out):
    os.makedirs(os.path.join(out, "ours"), exist_ok=True)
    spec_path = os.path.join(out, "spec.json")
    with open(spec_path, "w") as f:
        json.dump(spec_cases, f)
    env = dict(os.environ, GLASS_PARITY_SPEC=spec_path)
    subprocess.run(
        [
            "cargo",
            "test",
            "--release",
            "-p",
            "ossido",
            "--test",
            "glass_parity",
            "--",
            "--ignored",
            "--nocapture",
        ],
        cwd=REPO,
        env=env,
        check=True,
    )


def local_mask(surface, inset):
    """Pixels of the crop inside the shapes, eroded by `inset` (rectangles only)."""
    x0, y0, w, h = SURFACES[surface]["crop"]
    mask = np.zeros((h, w), bool)
    for x, y, rw, rh, r, _, tip in SURFACES[surface]["regions"]:
        if tip != "none":
            continue
        ys, xs = np.mgrid[0:h, 0:w]
        px, py = xs + x0 + 0.5, ys + y0 + 0.5
        cx, cy = x + rw / 2, y + rh / 2
        hw, hh = rw / 2 - inset, rh / 2 - inset
        rr = max(min(r, hw, hh) - inset, 0)
        dx = np.maximum(np.abs(px - cx) - (hw - rr), 0)
        dy = np.maximum(np.abs(py - cy) - (hh - rr), 0)
        inside = (np.abs(px - cx) <= hw) & (np.abs(py - cy) <= hh) & (dx * dx + dy * dy <= rr * rr)
        mask |= inside
    return mask


def rmse(a, b, mask):
    if mask.sum() == 0:
        return float("nan")
    d = (a - b)[mask]
    return float(np.sqrt((d * d).mean()))


def valid_mask(surface, shape):
    """Where the reference glass sees only the wallpaper (crop pixels)."""
    mask = np.ones(shape, bool)
    valid = SURFACES[surface].get("valid")
    if valid:
        x0, y0, x1, y1 = valid
        mask[:] = False
        mask[y0:y1, x0:x1] = True
    return mask


def clean_mask(ref_gray, mask):
    """Inside pixels where the reference is plain glass (no text, icons, highlights): every
    channel within 6 levels of the median fill on gray, at least 4 px from any other pixel."""
    if ref_gray is None or not mask.any():
        return mask
    med = np.median(ref_gray[mask], axis=0)
    plain = (np.abs(ref_gray - med).max(2) < 6) & mask
    bad = ~plain
    grown = bad.copy()
    for dy in range(-4, 5):
        for dx in range(-4, 5):
            grown |= np.roll(np.roll(bad, dy, 0), dx, 1)
    return mask & ~grown


def displacement(img, wall_axis, interior, gray=None):
    """Apparent source coordinate per pixel from a disp map capture (level = coord mod 256),
    calibrated on `interior` (where the displacement is ~0). With the capture over `gray`, its
    edge (rim, glow) is taken out first: a glow of 10 levels would read as ~20 px."""
    lum = img.mean(2)
    if gray is not None:
        g = gray.mean(2)
        lum = lum - (g - np.median(g[interior]))
    h, w = lum.shape
    coord = np.arange(w)[None, :].repeat(h, 0) if wall_axis == "x" else np.arange(h)[:, None].repeat(w, 1)
    level = coord % 256
    sel = interior & (level > 20) & (level < 235)
    g, o = np.polyfit(level[sel], lum[sel], 1)
    lvl = (lum - o) / g
    base = coord - level
    cands = np.stack([base - 256 + lvl, base + lvl, base + 256 + lvl])
    idx = np.argmin(np.abs(cands - coord[None]), 0)
    src = np.take_along_axis(cands, idx[None], 0)[0]
    return src - coord


def region_edges(img, wall, surface, region, axis):
    """The silhouette of one region: per row (axis 'x') or column ('y') of the region, the
    position of the steepest luminance step within 6 px of each nominal side (the edge's
    outline against the rim), first and last. A step is the edge only where it stands out:
    rows or columns whose strongest step is weaker than 12 levels are skipped (NaN), and so
    are those where the wallpaper itself has steps there."""
    x0, y0, _, _ = SURFACES[surface]["crop"]
    x, y, w, h = region[:4]
    lum, wl = img.mean(2), wall.mean(2)
    lo_x, lo_y = int(x - x0), int(y - y0)
    if axis == "y":
        lum, wl = lum.T, wl.T
        lo_x, lo_y, w, h = lo_y, lo_x, h, w
    n = lum.shape[1]
    first, last = [], []
    for i in range(max(lo_y, 0), min(lo_y + h, lum.shape[0])):
        step = np.abs(np.diff(lum[i]))
        wall_step = np.abs(np.diff(wl[i]))
        found = []
        for centre in (lo_x, lo_x + w):
            a, b = max(centre - 7, 0), min(centre + 6, n - 1)
            window = step[a:b]
            if len(window) == 0 or window.max() < 12 or wall_step[a:b].max() > 4:
                found.append(np.nan)
                continue
            # The step between pixels k and k + 1: the edge at k + 1.
            found.append(a + int(np.argmax(window)) + 1.0)
        first.append(found[0])
        last.append(found[1])
    return np.array(first, float), np.array(last, float)


def silhouette_error(ref, ours, wall, surface):
    errs = []
    for region in SURFACES[surface]["regions"]:
        if region[6] != "none":
            continue
        for axis in ("x", "y"):
            rf, rl = region_edges(ref, wall, surface, region, axis)
            of, ol = region_edges(ours, wall, surface, region, axis)
            for r, o in ((rf, of), (rl, ol)):
                ok = ~np.isnan(r) & ~np.isnan(o)
                errs.extend(np.abs(r[ok] - o[ok]).tolist())
    if not errs:
        return float("nan"), float("nan")
    e = np.array(errs)
    return float(e.mean()), float(e.max())


def tip_profile(img, wall, surface):
    """The tip of the first tipped region: per row from 3 px above its base to 4 px below its
    apex, the left and right edges of the glass within ±20 px of the tip axis; per column
    across the tip, the lowest glass row. Glass = the pixel is past halfway from the
    wallpaper to the fill (on the fill's side: dark outlines and shadows do not count)."""
    x0, y0, _, _ = SURFACES[surface]["crop"]
    tips = [t for t in SURFACES[surface]["regions"] if t[6] != "none"]
    if not tips:
        return None
    x, y, w, h, _, _, kind = tips[0]
    cx = x + w / 2 - x0
    lum, wl = img.mean(2), wall.mean(2)
    if kind == "down":
        fill_rows = slice(int(y - y0 - 20), int(y - y0 - 6))
    else:
        fill_rows = slice(int(y - y0 + h + 6), int(y - y0 + h + 20))
    fill = np.median(lum[fill_rows, int(cx) - 3 : int(cx) + 3])
    wall_level = np.median(wl[fill_rows, int(cx) - 3 : int(cx) + 3])
    sign = 1.0 if fill > wall_level else -1.0
    glass = (lum - wl) * sign > 0.5 * abs(fill - wall_level)
    rows = range(int(y - y0) - 3, int(y - y0 + h) + 4) if kind == "down" else range(int(y - y0) - 4, int(y - y0 + h) + 3)
    edges = []
    for r in rows:
        seg = glass[r, int(cx) - 20 : int(cx) + 21]
        idx = np.nonzero(seg)[0]
        edges.append((idx[0], idx[-1]) if len(idx) else (np.nan, np.nan))
    cols = []
    for c in range(int(cx) - 10, int(cx) + 11):
        seg = glass[int(y - y0) - 2 : int(y - y0 + h) + 6, c]
        idx = np.nonzero(seg)[0]
        cols.append(idx[-1] if len(idx) else np.nan)
    return np.array(edges, float), np.array(cols, float)


def tip_error(ref, ours, wall, surface):
    a, b = tip_profile(ref, wall, surface), tip_profile(ours, wall, surface)
    if a is None or b is None:
        return float("nan"), float("nan")
    e = np.concatenate([np.abs(a[0] - b[0]).ravel(), np.abs(a[1] - b[1])])
    e = e[~np.isnan(e)]
    return (float(e.mean()), float(e.max())) if len(e) else (float("nan"), float("nan"))


def rim_error(ref, ours, surface):
    """The edge of the first region on gray, through the middle of each clean side (the
    surface's `rim_sides`): RMSE of the luminance from the edge to 14 px inside, and mean
    difference of the outline's darkening 1 px outside (alpha of black against the
    background 3 px out, so shadows drawn by the shell do not count)."""
    x0, y0, _, _ = SURFACES[surface]["crop"]
    x, y, w, h = SURFACES[surface]["regions"][SURFACES[surface].get("rim_region", 0)][:4]
    cx, cy = x + w // 2 - x0, y + h // 2 - y0
    lx, ly = x - x0, y - y0
    rl, rr = ref.mean(2), ours.mean(2)
    H, W = rl.shape
    sides = SURFACES[surface].get("rim_sides", ("top", "bottom", "left", "right"))
    span = np.arange(-3, 15)
    inner, outline = [], []
    for side in sides:
        rows, cols = {
            "top": (ly + span, np.full_like(span, cx)),
            "bottom": (ly + h - 1 - span, np.full_like(span, cx)),
            "left": (np.full_like(span, cy), lx + span),
            "right": (np.full_like(span, cy), lx + w - 1 - span),
        }[side]
        rows, cols = np.clip(rows, 0, H - 1), np.clip(cols, 0, W - 1)
        a, b = rl[rows, cols], rr[rows, cols]
        inner.append(a[3:] - b[3:])
        outline.append(abs((1 - a[2] / max(a[0], 1)) - (1 - b[2] / max(b[0], 1))))
    d = np.concatenate(inner)
    return float(np.sqrt((d * d).mean())), float(np.mean(outline))


def analyze(ref, out, only):
    rows = []
    os.makedirs(os.path.join(out, "side"), exist_ok=True)
    for surface, spec in SURFACES.items():
        if only and surface not in only:
            continue
        inside = local_mask(surface, 10)
        inside &= valid_mask(surface, inside.shape)
        for ap in spec.get("appearances", ("light", "dark")):
            ref_gray = reference_crop(ref, surface, "gray", ap)
            # glass.md: the light Control Center mask comes from the dark gray capture (same
            # geometry; the light gray capture of the series has no Control Center).
            mask_from = spec.get("mask_appearance", ap)
            mask = clean_mask(reference_crop(ref, surface, "gray", mask_from), inside)
            # The blur away from the lens band (the Dock's lens reaches ~22 pt), where a
            # stripe's position does not depend on the refraction.
            deep = mask & local_mask(surface, 24)
            result = {"surface": surface, "appearance": ap}
            fills, blurs = [], []
            for wp in spec["files"]:
                path = os.path.join(out, "ours", f"{surface}-{wp}-{ap}.png")
                r = reference_crop(ref, surface, wp, ap)
                if r is None or not os.path.exists(path):
                    continue
                o = our_crop(path, surface)
                # The capture must show the same wallpaper outside the shapes: some series
                # were taken with the wallpaper shifted (cc stripes-v light).
                wall = wallpaper_crop(ref, surface, wp)
                dy = shifted(r, o, wall) if spec.get("check_shift") else 0
                if dy:
                    result.setdefault("misaligned", []).append(f"{wp} ({dy:+d} px)")
                    continue
                side = np.concatenate([r, o, np.clip(np.abs(r - o) * 4, 0, 255)], 1)
                Image.fromarray(side.astype(np.uint8)).save(
                    os.path.join(out, "side", f"{surface}-{wp}-{ap}.png")
                )
                if wp == "gray" or wp.startswith("saw"):
                    fills.append(rmse(r, o, mask))
                    result[f"fill_{wp}"] = fills[-1]
                if wp.startswith("stripes"):
                    blurs.append(rmse(r, o, deep))
                if wp.startswith("disp"):
                    axis = wp[-1]
                    mask_gray = reference_crop(ref, surface, "gray", spec.get("mask_appearance", ap))
                    plain = clean_mask(mask_gray, local_mask(surface, 0))
                    band = plain & ~local_mask(surface, 30)
                    calib = clean_mask(mask_gray, local_mask(surface, 34))
                    if calib.sum() > 200:
                        our_gray = our_crop(os.path.join(out, "ours", f"{surface}-gray-{ap}.png"), surface)
                        dr = displacement(r, axis, calib, mask_gray)
                        do = displacement(o, axis, calib, our_gray)
                        result[f"refraction_{axis}"] = rmse(dr[..., None], do[..., None], band)
                if wp == "gray":
                    wall = wallpaper_crop(ref, surface, wp)
                    result["silhouette_mean"], result["silhouette_max"] = silhouette_error(r, o, wall, surface)
                    result["rim"], result["outline"] = rim_error(r, o, surface)
                    if any(t[6] != "none" for t in spec["regions"]):
                        result["tip_mean"], result["tip_max"] = tip_error(r, o, wall, surface)
            if fills:
                result["fill"] = float(np.mean(fills))
            if blurs:
                result["blur"] = float(np.mean(blurs))
            rows.append(result)
    return rows


def shifted(r, o, wall):
    """The vertical offset of the capture's shapes from ours, in pixels (0 when aligned).

    Some series were taken with a notification above the Control Center, which moves every
    module down (cc stripes-v-mid light): the rows that differ from the wallpaper tell.
    """
    def profile(img):
        rows = (np.abs(img - wall).mean(2) > 12).mean(1)
        return rows - rows.mean()

    pr, po = profile(r), profile(o)
    scores = {dy: float(np.dot(np.roll(pr, dy), po)) for dy in range(-40, 41)}
    best = max(scores, key=scores.get)
    return best if scores[best] > 1.2 * scores[0] else 0


# TIP-01 cases: where the tip points, the half to use when the other runs into a body corner, the measured model (sharp half base, flank angle in degrees,
# apex radius, fillet radius) and the tip rectangle a client sends (width along the edge,
# depth).
TIP_CASES = {
    "dock": {
        "dir": "down",
        "half": "right",
        # The Dock shows at the bottom of the capture: only the rows above it.
        "rows": 44,
        "model": (10.817, 50.82, 5.295, 9.414),
        "size": (31, 10),
    },
    "popover-down": {
        "dir": "down",
        "model": (12.76, 46.2, 5.75, 8.05),
        "size": (32, 11),
    },
    "popover-up": {
        "dir": "up",
        "model": (12.73, 46.2, 5.41, 7.97),
        "size": (32, 11),
    },
    "popover-left": {
        "dir": "left",
        "model": (13.7, 46.1, 5.73, 13.6),
        "size": (39, 12),
    },
    "popover-right": {
        "dir": "right",
        "model": (13.7, 46.1, 5.73, 13.6),
        "size": (39, 12),
    },
}
ROTATION = {"down": 0, "up": 2, "left": 1, "right": 3}
# Where our body and tip go on the 2560 x 1440 screen: the body's edge facing the tip.
TIP_BODY = (1000, 500, 360, 300)


def load_tip_files(ref):
    """The capture of every tip case on black, from the private reference's table
    (`tools/glass_parity_files.json`, key `tips`: per case a path pattern with `{ap}`)."""
    path = os.path.join(ref, "tools", "glass_parity_files.json")
    if not os.path.exists(path):
        sys.exit(f"capture table not found: {path} (it comes with silica-reference)")
    with open(path) as f:
        table = json.load(f).get("tips", {})
    for case, spec in TIP_CASES.items():
        if case not in table:
            sys.exit(f"{path}: no entry for the tip case {case!r} under 'tips'")
        spec["file"] = table[case]


def tip_regions(case):
    """Body and tip rectangles for a case, and the apex position on screen."""
    x, y, w, h = TIP_BODY
    tw, td = TIP_CASES[case]["size"]
    d = TIP_CASES[case]["dir"]
    cx, cy = x + w // 2, y + h // 2
    tip = {
        "down": (cx - tw // 2, y + h, tw, td),
        "up": (cx - tw // 2, y - td, tw, td),
        "left": (x - td, cy - tw // 2, td, tw),
        "right": (x + w, cy - tw // 2, td, tw),
    }[d]
    body = (x, y, w, h, 12, "thick", "none")
    return [body, (*tip, 0, "thick", d)]


def tip_cases(ref, out):
    """Render cases: every tip over a black wallpaper, light and dark."""
    black = os.path.join(out, "black.png")
    os.makedirs(out, exist_ok=True)
    Image.new("RGB", (2560, 1440), (0, 0, 0)).save(black)
    result = []
    for case, spec in TIP_CASES.items():
        for ap in ("light", "dark"):
            if not os.path.exists(os.path.join(ref, spec["file"].format(ap=ap))):
                continue
            result.append(
                {
                    "name": f"tip-{case}-{ap}",
                    "dark": ap == "dark",
                    "wallpaper": black,
                    "out": os.path.join(out, "ours", f"tip-{case}-{ap}.png"),
                    "regions": [
                        {"x": x, "y": y, "w": w, "h": h, "r": r, "material": m, "tip": t}
                        for (x, y, w, h, r, m, t) in tip_regions(case)
                    ],
                }
            )
    return result


def our_tip_crop(path, case):
    """Our render around the tip, 80 across and 50 along (20 inside the body), unrotated."""
    image = load(path)
    tip = tip_regions(case)[1]
    x, y, w, h, _, _, d = tip
    if d == "down":
        return image[y - 20 : y + 30, x + w // 2 - 40 : x + w // 2 + 40]
    if d == "up":
        return image[y + h - 30 : y + h + 20, x + w // 2 - 40 : x + w // 2 + 40]
    if d == "left":
        return image[y + h // 2 - 40 : y + h // 2 + 40, x + w - 30 : x + w + 20]
    return image[y + h // 2 - 40 : y + h // 2 + 40, x - 20 : x + 30]


def cubic_zoom(a, z, axis):
    """Upsamples `a` by `z` along `axis` with a cubic (Catmull-Rom) through the pixel centers,
    like the reference tool's cubic zoom (without scipy)."""
    a = np.moveaxis(a, axis, 0)
    n = a.shape[0]
    t = (np.arange(n * z) + 0.5) / z - 0.5
    i = np.clip(np.floor(t).astype(int), 0, n - 1)
    f = (t - i)[:, None] if a.ndim > 1 else t - i
    p = [a[np.clip(i + k, 0, n - 1)] for k in (-1, 0, 1, 2)]
    out = 0.5 * (
        2 * p[1]
        + (-p[0] + p[2]) * f
        + (2 * p[0] - 5 * p[1] + 4 * p[2] - p[3]) * f * f
        + (-p[0] + 3 * p[1] - 3 * p[2] + p[3]) * f * f * f
    )
    return np.moveaxis(out, 0, axis)


def outline_profile(image, d, half=None, rows=None):
    """The outline of a tip on black, as the reference's tool takes it: coverage on the
    luminance against the background and the body's fill, the lowest 0.5 crossing per column
    (columns every 0.1 px, linear between pixels), depth from the body edge as a function of
    the distance from the apex's axis (both halves averaged, or one mirrored). Returns the grid,
    the depth, where the outline leaves the body edge, and the rotated coverage."""
    lum = image @ np.array([0.2126, 0.7152, 0.0722])
    lum = np.rot90(lum, k=ROTATION[d])
    if rows is not None:
        lum = lum[:rows]
    H, W = lum.shape
    # Black wallpaper behind every case.
    bg = 0.0
    fill = np.median(lum[2:6, W // 3 : 2 * W // 3])
    alpha = np.clip((lum - bg) / (fill - bg), 0, 1)
    z = 10
    up = cubic_zoom(cubic_zoom(alpha, z, 0), z, 1)
    xs = (np.arange(up.shape[1]) + 0.5) / z - 0.5
    ys = []
    for c in range(up.shape[1]):
        idx = np.nonzero(up[:, c] >= 0.5)[0]
        ys.append((idx.max() + 0.5) / z - 0.5 if len(idx) else np.nan)
    ys = np.array(ys)
    ok = ~np.isnan(ys)
    xs, ys = xs[ok], ys[ok]
    ends = ys[xs < 4] if half == "left" else ys[xs > W - 5] if half == "right" else np.r_[ys[xs < 4], ys[xs > W - 5]]
    depth = ys - np.median(ends)
    top = depth.max()
    sel = depth > 0.9 * top
    xa = np.sum(xs[sel] * depth[sel]) / np.sum(depth[sel])
    u = xs - xa
    g = np.arange(0, min(-u.min(), u.max()) - 0.05, 0.1)
    left = np.interp(-g, u[::-1] if u[0] > u[-1] else u, depth[::-1] if u[0] > u[-1] else depth)
    right = np.interp(g, u, depth)
    if half == "left":
        right = left
    if half == "right":
        left = right
    prof = (left + right) / 2
    leaves = g[np.nonzero(prof > 0.05)[0].max()] if np.any(prof > 0.05) else g[-1]
    return g, prof, float(leaves), alpha


def model_depth(model, x):
    """The measured model's depth at distance x from the axis (the reference's fit C)."""
    hb, angle, ra, rf = model
    al = np.radians(angle)
    ta = np.tan(al)
    yc = hb * ta - ra / np.cos(al)
    xa_t = ra * np.sin(al)
    xf = hb + rf * np.tan(al / 2)
    xf_t = xf - rf * np.sin(al)
    y = np.where(x <= xa_t, yc + np.sqrt(np.clip(ra**2 - x**2, 0, None)), (hb - x) * ta)
    y = np.where((x >= xf_t) & (x <= xf), rf - np.sqrt(np.clip(rf**2 - (x - xf) ** 2, 0, None)), y)
    return np.where(x > xf, 0.0, y)


def tip_side_by_side(ref_alpha, our_alpha, path):
    """Both coverages (tip pointing down) aligned on the apex, 4x nearest: reference left."""
    h, w = 24, 48

    def around_apex(a):
        covered = a >= 0.5
        rows = np.nonzero(covered.any(1))[0]
        apex_row = rows.max() if len(rows) else a.shape[0] - 1
        cols = np.nonzero(covered[apex_row])[0]
        apex_col = int(cols.mean()) if len(cols) else a.shape[1] // 2
        padded = np.pad(a, ((h, h), (w, w)))
        y0, x0 = apex_row + h - (h - 4), apex_col + w - w // 2
        return padded[y0 : y0 + h, x0 : x0 + w]

    both = np.concatenate([around_apex(ref_alpha), np.ones((h, 2)), around_apex(our_alpha)], 1)
    image = Image.fromarray((both * 255).astype(np.uint8)).resize((both.shape[1] * 4, h * 4), Image.NEAREST)
    image.save(path)


def analyze_tips(ref, out):
    os.makedirs(os.path.join(out, "tips"), exist_ok=True)
    rows = []
    for case, spec in TIP_CASES.items():
        for ap in ("light", "dark"):
            ref_path = os.path.join(ref, spec["file"].format(ap=ap))
            ours_path = os.path.join(out, "ours", f"tip-{case}-{ap}.png")
            if not (os.path.exists(ref_path) and os.path.exists(ours_path)):
                continue
            half = spec.get("half")
            gr, pr, wr, ar = outline_profile(load(ref_path), spec["dir"], half, spec.get("rows"))
            go, po, wo, ao = outline_profile(our_tip_crop(ours_path, case), spec["dir"])
            reach = min(gr[-1], go[-1], max(wr, wo) + 1.0)
            grid = np.arange(0, reach, 0.1)
            ref_d = np.interp(grid, gr, pr)
            our_d = np.interp(grid, go, po)
            model = model_depth(spec["model"], grid)
            vs_ref = np.abs(our_d - ref_d)
            # The body edge of a small capture is known to ~0.3 px: the residual after the
            # best constant offset tells the shape apart from the edge.
            inner = model > 0.5
            shift = float(np.median((our_d - ref_d)[inner])) if inner.any() else 0.0
            shape = np.abs(our_d - ref_d - shift)[inner] if inner.any() else np.zeros(1)
            vs_model = np.abs(our_d - model)
            ref_model = np.abs(ref_d - model)
            rows.append(
                {
                    "case": case,
                    "appearance": ap,
                    "depth_ref": float(pr.max()),
                    "depth_ours": float(po.max()),
                    "leaves_ref": wr,
                    "leaves_ours": wo,
                    "vs_ref_mean": float(vs_ref.mean()),
                    "vs_ref_max": float(vs_ref.max()),
                    "edge_shift": shift,
                    "shape_mean": float(shape.mean()),
                    "shape_max": float(shape.max()),
                    "vs_model_mean": float(vs_model.mean()),
                    "vs_model_max": float(vs_model.max()),
                    "ref_vs_model_max": float(ref_model.max()),
                }
            )
            tip_side_by_side(ar, ao, os.path.join(out, "tips", f"{case}-{ap}.png"))
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", default=os.path.join(REPO, "plan", "reference"))
    parser.add_argument("--out", default=os.path.join(REPO, "target", "glass-parity"))
    parser.add_argument("--only", nargs="*")
    parser.add_argument("--skip-render", action="store_true")
    parser.add_argument("--tips", action="store_true")
    args = parser.parse_args()
    args.out = os.path.abspath(args.out)
    args.reference = os.path.abspath(args.reference)
    if not os.path.isdir(args.reference):
        sys.exit(f"reference not found: {args.reference}")
    if args.tips:
        load_tip_files(args.reference)
        if not args.skip_render:
            render(tip_cases(args.reference, args.out), args.out)
        rows = analyze_tips(args.reference, args.out)
        keys = ["depth_ref", "depth_ours", "vs_ref_mean", "vs_ref_max", "edge_shift", "shape_mean", "shape_max", "vs_model_mean", "vs_model_max", "ref_vs_model_max"]
        print(f"{'case':14} {'ap':5} " + " ".join(f"{k:>16}" for k in keys))
        for row in rows:
            print(f"{row['case']:14} {row['appearance']:5} " + " ".join(f"{row[k]:16.2f}" for k in keys))
        with open(os.path.join(args.out, "tips.json"), "w") as f:
            json.dump(rows, f, indent=1)
        return
    load_files(args.reference)
    if not args.skip_render:
        render(cases(args.reference, args.out, args.only), args.out)
    rows = analyze(args.reference, args.out, args.only)
    keys = ["fill", "blur", "refraction_x", "refraction_y", "rim", "outline", "silhouette_mean", "silhouette_max", "tip_mean", "tip_max"]
    print(f"{'surface':10} {'ap':5} " + " ".join(f"{k:>14}" for k in keys))
    for row in rows:
        print(
            f"{row['surface']:10} {row['appearance']:5} "
            + " ".join(f"{row[k]:14.2f}" if k in row else f"{'-':>14}" for k in keys)
        )
    with open(os.path.join(args.out, "summary.json"), "w") as f:
        json.dump(rows, f, indent=1)


if __name__ == "__main__":
    main()
