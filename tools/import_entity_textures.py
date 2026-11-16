#!/usr/bin/env python3
"""Import the original atlas and dedicated sources; validate palette and alpha.

Usage: python3 tools/import_entity_textures.py [source/atlas32.png]
       [--only-simple | --only-crafting | --validate-only]
Python only orchestrates ImageMagick and writes metadata; image operations use magick.
"""
import argparse
import json
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "data/entity_tex"


def magick(*args):
    return subprocess.check_output(["magick", *map(str, args)])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", nargs="?", default=str(OUT / "source/atlas32.png"))
    parser.add_argument("--only-crafting", action="store_true", help="Import dedicated crafting sprites; preserve the original 64 PNGs")
    parser.add_argument("--only-simple", action="store_true", help="Update only the six simple terrain tiles; preserve other PNGs")
    parser.add_argument("--validate-only", action="store_true", help="Preserve sprites; validate and rebuild manifest/index/previews")
    options = parser.parse_args()
    assert not (options.only_simple and options.only_crafting), "Choose one import subset"
    source = Path(options.source).resolve()
    recipe_path = OUT / "source/simple_terrain/recipe.json"
    overrides = {t["id"]: t for t in json.loads(recipe_path.read_text())["tiles"]} if recipe_path.exists() else {}
    assert not options.only_simple or overrides, "Simple terrain recipe is missing"
    palette = json.loads((ROOT / "document/color_thema/palette.json").read_text())
    colors = {c["id"]: c["hex"] for c in palette["colors"]}
    allowed = {tuple(c["rgb"]) for c in palette["colors"]}
    assert len(colors) == len(allowed) == 32, "Expected 32 unique palette colors"
    gpl = (ROOT / "document/color_thema/palette.gpl").read_text()
    gpl_colors = [(tuple(map(int, m[:3])), m[3]) for m in re.findall(
        r"^\s*(\d+)\s+(\d+)\s+(\d+)\s+(\S+)\s*$", gpl, re.M)]
    assert gpl_colors == [(tuple(c["rgb"]), c["id"]) for c in palette["colors"]], "GPL/JSON mismatch"
    paths = re.findall(r'"([^"\n]+\.lua)"', (ROOT / "data/entity_templates/manifest.lua").read_text())
    aliases_path = OUT / "source/crafting_aliases.json"
    aliases = json.loads(aliases_path.read_text()) if aliases_path.exists() else {}
    dedicated_path = OUT / "source/crafting/recipe.json"
    dedicated = json.loads(dedicated_path.read_text())["tiles"] if dedicated_path.exists() else {}
    assert not (set(aliases) & set(dedicated)), "Alias/dedicated mappings overlap"
    assert len(paths) >= 64, "Original atlas definitions are required"
    assert len(paths) == 64 + len(aliases) + len(dedicated), "Additional definitions need an explicit source mapping"
    width, height = map(int, magick("identify", "-format", "%w %h", source).split())
    assert width == height
    OUT.mkdir(parents=True, exist_ok=True)
    entries = []
    opaque = {"terrain.field": "moss", "terrain.road": "bark", "terrain.pond": "deep_water",
              "terrain.wasteland": "peat", "obstacle.stone_wall": "charcoal",
              "obstacle.wooden_wall": "bark"}
    with tempfile.TemporaryDirectory() as tmp:
        swatches = Path(tmp) / "palette.ppm"
        swatches.write_text("P3\n32 1\n255\n" + "\n".join(" ".join(map(str, c["rgb"])) for c in palette["colors"]))
        for i, path in enumerate(paths):
            text = (ROOT / path).read_text()
            entity_id = re.search(r'\bid = "([^"]+)"', text)[1]
            category = re.search(r'\bcategory = "([^"]+)"', text)[1]
            name = re.search(r'\bname = "([^"]+)"', text)[1]
            target = OUT / category / f"{entity_id}.png"
            if entity_id in aliases:
                target = OUT / category / f"{aliases[entity_id]}.png"
            assert i < 64 or entity_id in aliases or entity_id in dedicated, entity_id
            target.parent.mkdir(exist_ok=True)
            x, y = round(i % 8 * width / 8), round(i // 8 * height / 8)
            w, h = round((i % 8 + 1) * width / 8)-x, round((i // 8 + 1) * height / 8)-y
            selected = ((not options.only_simple or entity_id in overrides)
                        and (not options.only_crafting or entity_id in dedicated))
            if not options.validate_only and entity_id not in aliases and selected:
                tile_source, crop, tile_swatches = source, f"{w}x{h}+{x}+{y}", swatches
                if entity_id in dedicated:
                    tile_source = OUT / dedicated[entity_id]["source"]
                    tw, th = map(int, magick("identify", "-format", "%w %h", tile_source).split())
                    crop = f"{tw}x{th}+0+0"
                if entity_id in overrides:
                    spec = overrides[entity_id]
                    tile_source = OUT / "source/simple_terrain" / spec["source"]
                    tw, th = map(int, magick("identify", "-format", "%w %h", tile_source).split())
                    crop = f"{tw}x{th}+0+0"
                    tile_swatches = Path(tmp) / "terrain.ppm"
                    keys = spec.get("palette_ids", [spec["base"], spec["accent"]])
                    assert 1 <= len(keys) <= 3, entity_id
                    rgb = [next(c["rgb"] for c in palette["colors"] if c["id"] == key) for key in keys]
                    tile_swatches.write_text(f"P3\n{len(rgb)} 1\n255\n" + "\n".join(" ".join(map(str, c)) for c in rgb))
                args = [tile_source, "-crop", crop, "+repage"]
                if entity_id in dedicated:
                    # Keep the generated object's proportions on a square transparent tile.
                    side = max(tw, th)
                    args += ["-background", "none", "-gravity", "center", "-extent", f"{side}x{side}"]
                args += ["-sample", "32x32!", "-channel", "A", "-threshold", "50%", "+channel"]
                if entity_id in opaque:
                    args += ["-background", colors[opaque[entity_id]], "-alpha", "remove", "-alpha", "on"]
                # remap also quantizes alpha; save and restore the binary mask explicitly.
                args += ["(", "+clone", "-alpha", "extract", "-write", "mpr:mask", "+delete", ")",
                         "-alpha", "off", "+dither", "-remap", tile_swatches, "mpr:mask", "-alpha", "off",
                         "-compose", "CopyOpacity", "-composite", "-depth", "8", f"PNG32:{target}"]
                magick(*args)
            assert magick("identify", "-format", "%w %h", target).decode() == "32 32", entity_id
            pixels = magick(target, "-depth", "8", "rgba:-")
            assert len(pixels) == 32 * 32 * 4
            rgba = list(zip(*[iter(pixels)] * 4))
            assert all(a in (0, 255) for *_, a in rgba), entity_id
            assert all((r, g, b) in allowed for r, g, b, a in rgba if a), entity_id
            if entity_id in overrides:
                spec = overrides[entity_id]
                base = next(tuple(c["rgb"]) for c in palette["colors"] if c["id"] == spec["base"])
                details = sum((r, g, b) != base for r, g, b, a in rgba)
                assert len({p[:3] for p in rgba}) <= 3, entity_id
                keys = spec.get("palette_ids", [spec["base"], spec["accent"]])
                tile_colors = {tuple(c["rgb"]) for c in palette["colors"] if c["id"] in keys}
                assert {p[:3] for p in rgba} <= tile_colors, entity_id
                assert details <= spec.get("max_detail_pixels", 102), f"Too much terrain detail: {entity_id}: {details}/1024"
                print(f"{entity_id}: {details}/1024 detail pixels")
            visible = sum(a == 255 for *_, a in rgba)
            assert visible > 0, f"Empty: {entity_id}"
            if entity_id in opaque:
                assert visible == 1024, entity_id
            else:
                assert visible < 1024, f"Missing transparency: {entity_id}"
            entries.append(dict(id=entity_id, name=name, category=category, definition=path,
                                file=target.relative_to(OUT).as_posix(), atlas_cell=[i % 8, i // 8],
                                opaque_pixels=visible, transparent_pixels=1024-visible))
            if entity_id in dedicated:
                entries[-1]["source_override"] = dedicated[entity_id]["source"]
                entries[-1]["dedicated_texture"] = True
            if entity_id in aliases:
                entries[-1]["source_alias"] = aliases[entity_id]
                entries[-1]["dedicated_texture"] = False
            if entity_id in overrides:
                entries[-1]["source_override"] = "source/simple_terrain/" + overrides[entity_id]["source"]
                spec = overrides[entity_id]
                entries[-1]["palette_ids"] = spec.get("palette_ids", [spec["base"], spec["accent"]])
        atlas = OUT / "atlas.png"
        magick("montage", *[OUT / e["file"] for e in entries], "-tile", "8x", "-geometry", "32x32+0+0",
               "-background", "none", f"PNG32:{atlas}")
        magick(atlas, "-filter", "point", "-resize", "400%", OUT / "preview.png")
        if dedicated:
            crafting_contact = OUT / "source/crafting/contact32.png"
            magick("montage", *[OUT / e["file"] for e in entries if e["id"] in dedicated],
                   "-tile", "7x", "-geometry", "32x32+2+2", "-background", "#1a2023", crafting_contact)
            magick(crafting_contact, "-filter", "point", "-resize", "800%", OUT / "crafting_preview.png")
        terrain_preview = Path(tmp) / "terrain-preview.png"
        magick("montage", *[OUT / e["file"] for e in entries if e["id"] in overrides],
               "-tile", "3x2", "-geometry", "32x32+2+2", "-background", "#0e1113", terrain_preview)
        magick(terrain_preview, "-filter", "point", "-resize", "800%", OUT / "terrain_preview.png")
        composites = []
        for i, entry in enumerate(entries):
            background = OUT / "static" / ("terrain.field.png" if i % 2 == 0 else "terrain.road.png")
            composite = Path(tmp) / f"composite-{i:02}.png"
            magick(background, OUT / entry["file"], "-compose", "Over", "-composite", composite)
            front = magick(OUT / entry["file"], "-depth", "8", "rgba:-")
            back = magick(background, "-depth", "8", "rgba:-")
            combined = magick(composite, "-depth", "8", "rgba:-")
            for offset in range(0, len(front), 4):
                if front[offset+3] == 0:
                    assert combined[offset:offset+4] == back[offset:offset+4], entry["id"]
            composites.append(composite)
        composite_atlas = Path(tmp) / "composite-atlas.png"
        magick("montage", *composites, "-tile", "8x", "-geometry", "32x32+0+0",
               "-background", "none", composite_atlas)
        magick(composite_atlas, "-filter", "point", "-resize", "400%", OUT / "composite_preview.png")
        if dedicated:
            crafting_composite = Path(tmp) / "crafting-composite.png"
            magick("montage", *[p for e, p in zip(entries, composites) if e["id"] in dedicated],
                   "-tile", "7x", "-geometry", "32x32+2+2", "-background", "#1a2023", crafting_composite)
            magick(crafting_composite, "-filter", "point", "-resize", "800%", OUT / "crafting_composite_preview.png")
    metadata = dict(tile_size=[32, 32], palette="../../document/color_thema/palette.json",
                    atlas="atlas.png", atlas_columns=8, entries=entries)
    (OUT / "manifest.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
    cards = []
    for e in entries:
        cards.append(f'<figure><div class="tile"><img src="{e["file"]}"></div>'
                     f'<figcaption>{e["name"]}<br><code>{e["id"]}</code>{"<br>既存画像の仮流用" if "source_alias" in e else ""}</figcaption></figure>')
    html = '''<!doctype html><html lang="ja"><meta charset="utf-8"><title>Entity textures · Ash & Rust 32</title>
<style>body{background:#0e1113;color:#d0cdbb;font:16px sans-serif;margin:32px}h1{color:#f0e6cc}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:12px}
figure{margin:0;padding:12px;background:#1a2023}img{width:128px;height:128px;image-rendering:pixelated}
.tile{width:128px;height:128px;background:repeating-conic-gradient(#465256 0% 25%,#2b3335 0% 50%) 0/32px 32px}
body[data-bg="field"] .tile{background:url(static/terrain.field.png);background-size:128px}
body[data-bg="road"] .tile{background:url(static/terrain.road.png);background-size:128px}
figcaption{margin-top:8px}code{font-size:12px}button{margin:0 8px 20px 0;padding:8px}</style>
<h1>Entity textures · Ash & Rust 32</h1><p>全{count}定義 / 32×32 RGBA / 4倍表示。背景を切り替えて透過を確認。</p>
<button onclick="document.body.dataset.bg=''">透過チェッカー</button>
<button onclick="document.body.dataset.bg='field'">野原に重ねる</button>
<button onclick="document.body.dataset.bg='road'">道に重ねる</button>
<div class="grid">''' + "\n".join(cards) + "</div></html>\n"
    html = html.replace("{count}", str(len(entries)))
    (OUT / "index.html").write_text(html)
    print(f"Validated {len(entries)} sprites: 32x32 RGBA, binary alpha, palette-only visible RGB")
    print("Full tiles:", len(opaque), "/ transparent cutouts:", len(entries)-len(opaque))


if __name__ == "__main__":
    main()
