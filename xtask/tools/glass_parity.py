#!/usr/bin/env python3
"""Glass parity: renders our glass over the reference wallpapers, at the same places as the
reference captures, and measures the difference.

Usage: glass_parity.py [--reference DIR] [--out DIR] [--only SURFACE] [--skip-render]

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
    # tip centered at x 856, 10 pt deep: silica_ui sends a 24 × 10 rectangle, the sharp
    # triangle whose fillets and round apex give the measured ~22 pt visible base).
    "dockmenu": {
        "crop": (780, 800, 340, 600),
        "regions": [
            (829, 841, 228, 497, 12, "thick", "none"),
            (844, 1338, 24, 10, 0, "thick", "down"),
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", default=os.path.join(REPO, "plan", "reference"))
    parser.add_argument("--out", default=os.path.join(REPO, "target", "glass-parity"))
    parser.add_argument("--only", nargs="*")
    parser.add_argument("--skip-render", action="store_true")
    args = parser.parse_args()
    args.out = os.path.abspath(args.out)
    args.reference = os.path.abspath(args.reference)
    if not os.path.isdir(args.reference):
        sys.exit(f"reference not found: {args.reference}")
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
