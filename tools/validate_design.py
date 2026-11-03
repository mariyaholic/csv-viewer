# SPDX-License-Identifier: Apache-2.0
"""Validate viewer design aliases, contrast, states and current palette parity.

Standard-library only. Run --help for portable token and sibling-snapshot paths.
Contrast uses WCAG relative luminance; this is not rendered/native accessibility proof.
"""
import argparse
import json
import re
from pathlib import Path

IDS = {'paper', 'ink', 'pc98', 'doloris', 'sumimi', 'marigold'}
ROLES = {
    'surface.canvas', 'surface.panel', 'surface.input', 'surface.hover',
    'surface.pressed', 'text.primary', 'text.secondary', 'text.info',
    'border.control', 'border.decorative', 'focus.ring', 'selection.fill',
    'selection.text', 'selection.outline', 'action.fill', 'action.text',
    'status.success', 'status.warning', 'status.error', 'ornament.trim',
    'control.highlight', 'control.shadow',
    'surface.control', 'surface.control-hover', 'surface.control-pressed',
}
STATES = {'empty', 'loading', 'ready', 'partial', 'error', 'cancelled', 'stale'}
CONTROL_STATES = {'default', 'hover', 'pressed', 'focused', 'disabled', 'busy', 'invalid'}


def luminance(color):
    values = [int(color[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    values = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in values]
    return sum(v * weight for v, weight in zip(values, (0.2126, 0.7152, 0.0722)))


def contrast(a, b):
    lo, hi = sorted((luminance(a), luminance(b)))
    return (hi + 0.05) / (lo + 0.05)


def resolve(data, theme, layer, name, trail=()):
    key = layer + '.' + name
    if key in trail:
        raise ValueError('Alias cycle: ' + ' -> '.join((*trail, key)))
    table = data['themes'][theme]['reference'] if layer == 'reference' else data.get(layer, {})
    if name not in table:
        raise ValueError('Missing alias: ' + key)
    value = table[name]
    if not isinstance(value, str):
        raise ValueError('Token must be a string: ' + key)
    if layer == 'reference':
        if not re.fullmatch(r'#[0-9a-fA-F]{6}', value):
            raise ValueError('Expected opaque RGB hex: ' + key)
        return value.lower()
    alias = re.fullmatch(r'\{(reference|semantic)\.(.+)\}', value)
    if not alias:
        raise ValueError('Non-alias component/semantic token: ' + key)
    next_layer, next_name = alias.groups()
    # Check cycles before tier constraints so malformed graphs cannot recurse forever.
    if next_layer + '.' + next_name in (*trail, key):
        raise ValueError('Alias cycle at ' + key)
    expected = 'reference' if layer == 'semantic' else 'semantic'
    if next_layer != expected:
        raise ValueError('Layer bypass at ' + key)
    return resolve(data, theme, next_layer, next_name, (*trail, key))


def validate(data):
    required_components = {'input.fill', 'command.hover.fill', 'command.boundary', 'grid.header.fill', 'input.invalid', 'message.warning', 'message.error', 'section.trim', 'coverage.text', 'command.pressed.fill', 'grid.selected.fill', 'message.success', 'grid.text', 'input.placeholder', 'grid.fill', 'focus.outline', 'command.primary.text', 'input.boundary', 'grid.selected.text', 'grid.selected.outline', 'input.text', 'inspector.fill', 'grid.header.text', 'tabs.text', 'grid.rule', 'command.primary.fill', 'inspector.text', 'tabs.active', 'command.secondary.text', 'command.secondary.fill', 'inspector.label'}
    if not required_components <= set(data['components']):
        raise ValueError('Missing component binding')
    scale = data['scale']
    spacing = scale['spacing_dip']
    if not spacing or any(type(n) not in (int, float) or n <= 0 for n in spacing) or spacing != sorted(set(spacing)):
        raise ValueError('Spacing must be positive, unique and increasing')
    for theme in data['themes'].values():
        for role in ('heading', 'ui', 'data'):
            if not isinstance(theme['typography'].get(role), str) or not theme['typography'][role].strip():
                raise ValueError('Missing typography face: ' + role)
    if scale['motion_ms']['reduced'] != 0:
        raise ValueError('Reduced motion must disable transitions')
    if set(data['themes']) != IDS:
        raise ValueError('Exactly the six established themes are required')
    if not ROLES <= set(data['semantic']):
        raise ValueError('Missing semantic role')
    for name, expected in [('states', STATES), ('control_states', CONTROL_STATES)]:
        if set(data[name]) != expected or len(data[name]) != len(expected):
            raise ValueError('Missing/duplicate ' + name)
    reference_keys = None
    report = {}
    for theme, entry in data['themes'].items():
        keys = set(entry['reference'])
        if reference_keys is not None and keys != reference_keys:
            raise ValueError('Reference role drift: ' + theme)
        reference_keys = keys
        for layer in ('reference', 'semantic', 'components'):
            table = entry[layer] if layer == 'reference' else data[layer]
            if len({k.casefold() for k in table}) != len(table):
                raise ValueError('Case collision in ' + layer)
            for name in table:
                resolve(data, theme, layer, name)
        def color(name):
            return resolve(data, theme, 'semantic', name)
        pairs = []
        for bg in ('surface.canvas', 'surface.panel'):
            for fg in ('text.primary', 'text.secondary', 'text.info',
                       'status.success', 'status.warning', 'status.error'):
                pairs.append((fg, bg, 4.5))
        for bg in ('surface.control', 'surface.control-hover', 'surface.control-pressed'):
            pairs.extend([('text.primary', bg, 4.5), ('focus.ring', bg, 3)])
        pairs.append(('text.secondary', 'surface.control', 4.5))
        pairs += [
            ('text.primary', 'surface.input', 4.5),
            ('text.secondary', 'surface.input', 4.5),
            ('text.primary', 'surface.hover', 4.5),
            ('text.primary', 'surface.pressed', 4.5),
            ('selection.text', 'selection.fill', 4.5),
            ('action.text', 'action.fill', 4.5),
            ('border.control', 'surface.panel', 3),
            ('border.control', 'surface.input', 3),
            ('border.control', 'surface.canvas', 3),
            ('focus.ring', 'surface.canvas', 3),
            ('focus.ring', 'surface.panel', 3),
            ('focus.ring', 'surface.input', 3),
            ('selection.outline', 'selection.fill', 3),
        ]
        checks = []
        for fg, bg, minimum in pairs:
            ratio = contrast(color(fg), color(bg))
            if ratio < minimum:
                raise ValueError(f'{theme}: {fg} on {bg}: {ratio:.3f} < {minimum}')
            checks.append({'foreground': fg, 'background': bg, 'minimum': minimum,
                           'ratio': round(ratio, 4)})
        report[theme] = checks
    return report


def check_runtime(data, path):
    source = Path(path).read_text(encoding='utf-8')
    found = set()
    for block in re.findall(r'Theme \{(.*?)\n    \}', source, re.S):
        match = re.search(r'id: "([^"]+)"', block)
        if not match:
            continue
        theme = match[1]
        found.add(theme)
        pairs = re.findall(r'(background|panel|text|muted|accent|info): 0x([0-9a-f]+)', block)
        if len(pairs) != 6:
            raise ValueError('Runtime palette shape changed: ' + theme)
        for name, value in pairs:
            expected = f'#{int(value, 16):06x}'
            if data['themes'][theme]['reference'][name] != expected:
                raise ValueError('Current palette drift: ' + theme + '.' + name)
    if found != IDS:
        raise ValueError('Runtime catalog shape changed; review extractor')


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tokens', type=Path, default=root / 'design/system/tokens.json')
    parser.add_argument('--peer', type=Path, help='Require JSON parity with another token snapshot')
    parser.add_argument('--report', type=Path, help='Write measured foreground/background contrast pairs')
    args = parser.parse_args()
    data = json.loads(args.tokens.read_text(encoding='utf-8'))
    report = validate(data)
    check_runtime(data, root / 'crates/viewer-shell/src/lib.rs')
    if args.peer and data != json.loads(args.peer.read_text(encoding='utf-8')):
        raise ValueError('Peer token snapshot differs')
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    count = sum(len(pairs) for pairs in report.values())
    print(f'PASS: {len(report)} themes, {count} contrast pairs, aliases, states, runtime palette parity')


if __name__ == '__main__':
    main()
