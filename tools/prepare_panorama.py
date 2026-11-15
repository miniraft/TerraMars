#!/usr/bin/env python3
"""Prepare the decorative NASA PIA23623 background, offline (requires Pillow).

Download the source URL in panorama_source.json, then pass its local path.
The geographic/scientific map rasters are never processed by this script.
"""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageEnhance, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
SOURCE = 'https://assets.science.nasa.gov/dynamicimage/assets/science/psd/photojournal/pia/pia23/pia23623/PIA23623.jpg?w=3000'

def prepare(source, blur_radius=3.0):
    original = Image.open(source).convert('RGB')
    original = original.resize((3000, round(original.height * 3000 / original.width)), Image.Resampling.LANCZOS)
    # A terrain-only rectangle excludes mosaic gaps and rover hardware.
    cropped = original.crop((0, 155, 1650, 470))
    cropped.save(ROOT / 'assets/maps/mars_panorama.jpg', quality=92)
    image = cropped.resize((1800, 342), Image.Resampling.LANCZOS)
    # A warm decorative grade; never used for instrument/map visualization.
    red, green, blue = image.split()
    image = Image.merge('RGB', (red.point(lambda v: min(255, v * 1.22)), green.point(lambda v: v * .80), blue.point(lambda v: v * .65)))
    image = ImageEnhance.Color(image).enhance(1.15)
    overlap = 140
    tail, head = image.crop((1660, 0, 1800, 342)), image.crop((0, 0, 140, 342))
    ramp = Image.new('L', (overlap, 342))
    ramp.putdata([round(255 * x / (overlap - 1)) for _ in range(342) for x in range(overlap)])
    image.paste(Image.composite(head, tail, ramp), (0, 0))
    image = image.crop((0, 0, 1660, 342))
    tiled = Image.new('RGB', (4980, 342))
    for x in (0, 1660, 3320):
        tiled.paste(image, (x, 0))
    image = tiled.filter(ImageFilter.GaussianBlur(blur_radius)).crop((1660, 0, 3320, 342))
    # The repeated texture has identical seam columns, including after blur.
    edge = Image.blend(image.crop((0, 0, 1, 342)), image.crop((1659, 0, 1660, 342)), .5)
    image.paste(edge, (0, 0)); image.paste(edge, (1659, 0))
    image.save(ROOT / 'assets/maps/mars_panorama_blurred.png')
    manifest = {
        'title': "Curiosity's 1.8-Billion-Pixel Panorama — terrain crop",
        'source': 'https://science.nasa.gov/photojournal/curiositys-18-billion-pixel-panorama/',
        'download': SOURCE, 'credit': 'NASA/JPL-Caltech/MSSS',
        'observed': '2019-11-24 through 2019-12-01', 'published': '2020-03-04',
        'blur_radius_pixels': blur_radius,
        'notes': 'Real NASA terrain crop. Decorative warm red grade, overlapping periodic crossfade and offline Gaussian blur. No runtime panorama blur; scientific maps remain unchanged.',
    }
    for key, name in [('source_asset', 'mars_panorama.jpg'), ('decorative_blurred_asset', 'mars_panorama_blurred.png')]:
        path = ROOT / 'assets/maps' / name
        manifest[key] = {'asset': str(path.relative_to(ROOT)), 'asset_size': list(Image.open(path).size), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    (ROOT / 'assets/maps/panorama_source.json').write_text(json.dumps(manifest, indent=2) + '\n')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--blur-radius', type=float, default=3.0,
                        help='Gaussian blur radius in source pixels (default: 3)')
    args = parser.parse_args()
    if not 0 <= args.blur_radius <= 100:
        parser.error('--blur-radius must be between 0 and 100')
    prepare(args.source, args.blur_radius)
