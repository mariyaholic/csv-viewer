# SPDX-License-Identifier: Apache-2.0
# /// script
# requires-python = ">=3.10"
# dependencies = ["Pillow==9.4.0"]
# ///
"""Render illustrative future-component plates, never runtime screenshots.

Requires Pillow 9.4 and the repo's licensed fonts. --help lists output options.
Windows font substitutions are listed in the design documentation; do not use these plates
as native-control, accessibility or runtime acceptance evidence.
"""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont
from validate_design import resolve, validate


def main():
    repo = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tokens', type=Path, default=repo / 'design/system/tokens.json')
    parser.add_argument('--output', type=Path, default=repo / 'design/system')
    args = parser.parse_args()
    data = json.loads(args.tokens.read_text())
    validate(data)
    args.output.mkdir(parents=True, exist_ok=True)
    fonts = repo / 'assets/desktop-fonts'
    sans = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
    mono = '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf'
    pc = '/usr/share/fonts/opentype/unifont/unifont.otf'
    plates = []
    for theme, entry in data['themes'].items():
        def c(role):
            return resolve(data, theme, 'semantic', role)
        ui_path = fonts / 'Outfit.ttf' if theme in ('doloris', 'sumimi', 'marigold') else (pc if theme == 'pc98' else sans)
        mono_path = fonts / 'JetBrainsMono.ttf' if theme in ('doloris', 'sumimi', 'marigold') else (pc if theme == 'pc98' else mono)
        title_path = fonts / 'Cinzel.ttf' if theme in ('doloris', 'sumimi', 'marigold') else ui_path
        def face(path, size, weight=400):
            font = ImageFont.truetype(str(path), size)
            try:
                axes = font.get_variation_axes()
                settings = [max(a['minimum'], min(weight, a['maximum']))
                            if a['name'] == b'Weight' else a['default'] for a in axes]
                font.set_variation_by_axes(settings)
            except OSError:
                pass  # Static fallback font: no variation axes exist.
            return font
        ui = face(ui_path, 20)
        small = face(ui_path, 17)
        code = face(mono_path, 19)
        heading = face(title_path, 34, 600)
        im = Image.new('RGB', (1500, 1110), c('surface.canvas'))
        draw = ImageDraw.Draw(im)

        def text(x, y, value, role='text.primary', font=None):
            draw.text((x, y), value, fill=c(role), font=font or ui)

        def box(rect, fill='surface.panel', border='border.control', width=1):
            draw.rectangle(rect, fill=c(fill), outline=c(border), width=width)

        def button(x, y, width, label, state='default'):
            fill = {'primary': 'action.fill', 'hover': 'surface.hover', 'pressed': 'surface.pressed'}.get(state, 'surface.panel')
            box((x, y, x + width, y + 42), fill)
            if state == 'focused':
                draw.rectangle((x - 4, y - 4, x + width + 4, y + 46), outline=c('focus.ring'), width=2)
            text(x + 12, y + 9, label, 'action.text' if state == 'primary' else ('text.secondary' if state == 'disabled' else 'text.primary'))

        text(32, 20, 'Design specimen · illustrative data', 'text.secondary', small)
        text(32, 65, entry['name'], font=heading)
        draw.line((32, 151, 1468, 151), fill=c('ornament.trim'), width=2)
        button(32, 174, 130, 'Open file', 'primary')
        button(176, 174, 110, 'Reload')
        button(300, 174, 190, 'Scan full file')
        button(504, 174, 150, 'Export…', 'disabled')
        text(32, 247, 'Find in shown sample', 'text.secondary', small)
        box((32, 278, 805, 324), 'surface.input')
        text(47, 290, 'Account begins with 001', font=code)
        button(825, 280, 130, 'Clear')
        text(1014, 286, '6 shown rows', 'text.secondary')
        box((32, 356, 978, 738))
        box((32, 356, 978, 398), 'surface.canvas')
        for x, label in [(48, 'Account'), (350, 'Symbol'), (840, 'Amount')]:
            text(x, 368, label, 'text.secondary', small)
        rows = [('001024', 'EURUSD', '-42.00'),
                ('001025', 'XAUUSD', '120.50'),
                ('001026', 'GBPUSD', '0.00'),
                ('001027', 'USDJPY', ''),
                ('001028', '<null>', '<null>'),
                ('001029', '"null"', '17.00')]
        for n, row in enumerate(rows):
            y = 399 + n * 49
            if n == 1:
                box((34, y, 976, y + 48), 'selection.fill', 'selection.outline', 2)
            for x, val in zip((48, 350, 840), row[:3]):
                if x == 840:
                    draw.text((950, y + 13), val, anchor='ra', fill=c('text.primary'), font=code)
                else:
                    text(x, y + 12, val, font=code)
            if n != 1:
                draw.line((36, y + 48, 974, y + 48), fill=c('border.decorative'))
        text(48, 709, 'Sample only', 'text.info', small)
        box((1000, 356, 1468, 738))
        text(1018, 373, 'Value     Schema     Diagnostics', 'text.secondary')
        draw.line((1018, 401, 1075, 401), fill=c('focus.ring'), width=2)
        text(1018, 426, 'Selected cell / Account', font=ui)
        text(1018, 466, 'CSV · source text', 'text.secondary', small)
        text(1018, 508, '001025', font=face(mono_path, 34))
        button(1018, 656, 214, 'Copy shown text')
        box((32, 760, 1468, 820), 'surface.panel')
        text(48, 778, 'PARTIAL', 'status.warning')
        text(160, 778, 'Sample search only.')
        text(32, 852, 'Control states', 'text.secondary')
        for x, width, label, state in [(32, 132, 'Default', 'default'), (184, 130, 'Hover', 'hover'),
                                       (334, 132, 'Pressed', 'pressed'), (486, 132, 'Focus', 'focused'),
                                       (638, 150, 'Disabled', 'disabled')]:
            button(x, 894, width, label, state)
        text(825, 896, 'Error: invalid filter', 'status.error')
        text(825, 926, 'Check field and operator.', 'text.secondary', small)
        text(32, 973, 'SUCCESS · scan verified', 'status.success')
        text(400, 973, 'INFO · metadata unverified', 'text.info')
        text(855, 973, 'CANCELLED · previous result', 'text.secondary')
        path = args.output / ('components-' + theme + '.png')
        im.save(path)
        plates.append(im)
        print(path)
    sheet = Image.new('RGB', (1500, 740), '#202029')
    for n, im in enumerate(plates):
        thumb = im.copy()
        thumb.thumbnail((500, 370))
        sheet.paste(thumb, ((n % 3) * 500, (n // 3) * 370))
    sheet.save(args.output / 'six-system-faces.png')


if __name__ == '__main__':
    main()
