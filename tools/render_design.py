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

        def button(x, y, width, label, state='default', primary=False, menu=False, split=False):
            # Material changes never substitute for an accessible state/name.
            radius = entry['button_material']['radius_dip']
            disabled = state == 'disabled'
            pressed = state == 'pressed'
            offset = data['scale']['button_pressed_offset_dip'] if pressed else 0
            elevation = 0 if disabled or pressed else data['scale']['button_elevation_dip']
            fill = ('action.fill' if primary else
                    {'hover': 'surface.control-hover', 'pressed': 'surface.control-pressed'}.get(state, 'surface.control'))
            rect = (x, y + offset, x + width, y + 42 + offset)
            if elevation:
                draw.rounded_rectangle((x, y + elevation, x + width, y + 42 + elevation),
                                       radius=radius, fill=c('control.shadow'))
            draw.rounded_rectangle(rect, radius=radius, fill=c(fill), outline=c('border.control'))
            if not disabled:
                top = 'control.shadow' if pressed else 'control.highlight'
                bottom = 'control.highlight' if pressed else 'control.shadow'
                draw.line((x + radius + 2, y + offset + 2, x + width - radius - 2, y + offset + 2), fill=c(top))
                draw.line((x + radius + 2, y + offset + 40, x + width - radius - 2, y + offset + 40), fill=c(bottom))
                if entry['button_material']['profile'] == 'fine-frame':
                    draw.rounded_rectangle((x + 3, y + offset + 3, x + width - 3, y + offset + 39),
                                           radius=1, outline=c('control.highlight'))
                if entry['button_material']['profile'] == 'warm-rim' and not pressed:
                    draw.line((x + radius + 2, y + 38, x + width - radius - 2, y + 38), fill=c('control.highlight'))
                if entry['button_material']['profile'] == 'hard-bevel':
                    draw.line((x + 2, y + offset + 2, x + 2, y + offset + 39), fill=c(top))
                    draw.line((x + width - 2, y + offset + 2, x + width - 2, y + offset + 39), fill=c(bottom))
            if state == 'focused':
                draw.rounded_rectangle((x - 4, y - 4, x + width + 4, y + 46), radius=radius + 3,
                                       outline=c('focus.ring'), width=2)
            foreground = c('text.secondary' if disabled else ('action.text' if primary else 'text.primary'))
            trailing = 28 if menu or split else 0
            bounds = draw.textbbox((0, 0), label, font=ui)
            label_width = bounds[2] - bounds[0]
            if label_width + trailing + 24 > width:
                raise ValueError('Button label needs more room: ' + label)
            draw.text((x + (width - trailing - label_width) / 2, y + 9 + offset), label, fill=foreground, font=ui)
            if menu or split:
                cx = x + width - 15
                cy = y + 21 + offset
                if split:
                    draw.line((x + width - 31, y + 6, x + width - 31, y + 36), fill=c('border.control'))
                draw.line((cx - 4, cy - 2, cx, cy + 2, cx + 4, cy - 2), fill=foreground, width=2)

        text(32, 20, 'Design specimen · illustrative data', 'text.secondary', small)
        text(32, 65, entry['name'], font=heading)
        draw.line((32, 151, 1468, 151), fill=c('ornament.trim'), width=2)
        button(32, 174, 142, 'Open file…')
        button(188, 174, 120, 'Reload')
        button(322, 174, 190, 'Scan full file')
        button(526, 174, 150, 'Export', menu=True)
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
        for x, width, label, state in [(32, 132, 'Rest', 'default'), (184, 130, 'Hover', 'hover'),
                                       (334, 132, 'Pressed', 'pressed'), (486, 132, 'Focus', 'focused'),
                                       (638, 150, 'Disabled', 'disabled')]:
            button(x, 894, width, label, state)
        button(810, 894, 140, 'Primary', primary=True)
        button(974, 894, 175, 'Scanning', state='busy')
        text(1180, 896, 'Invalid filter', 'status.error')
        button(810, 957, 140, 'Copy', split=True)
        button(974, 957, 175, 'Cancel')
        text(32, 1040, 'SUCCESS · scan verified', 'status.success')
        text(400, 1040, 'INFO · metadata unverified', 'text.info')
        text(855, 1040, 'CANCELLED · previous result', 'text.secondary')
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
