#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Build/check the offline guide from the project's small, explicit Markdown subset.

No install/network dependency. Supports headings, paragraphs, flat lists, fenced
code, inline code and bold. It is not a general Markdown/HTML renderer: text is
escaped, no raw HTML is enabled, and no external resources are generated.
"""
import argparse
import html
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def inline(text):
    parts = re.split(r'(`[^`]+`|\*\*[^*]+\*\*)', text)
    result = []
    for part in parts:
        if part.startswith('`') and part.endswith('`'):
            result.append('<code>' + html.escape(part[1:-1]) + '</code>')
        elif part.startswith('**') and part.endswith('**'):
            result.append('<strong>' + html.escape(part[2:-2]) + '</strong>')
        else:
            result.append(html.escape(part))
    return ''.join(result)


def render(source):
    tokens = json.loads((ROOT / 'design/system/tokens.json').read_text(encoding='utf-8'))
    paper = tokens['themes']['paper']['reference']
    ink = tokens['themes']['ink']['reference']
    body, toc, paragraph, code = [], [], [], None
    listing, title, ids = None, 'User guide', {}

    def flush():
        nonlocal listing
        if paragraph:
            body.append('<p>' + inline(' '.join(paragraph)) + '</p>')
            paragraph.clear()
        if listing:
            body.append('</' + listing + '>')
            listing = None

    for line in source.splitlines():
        if line.startswith('```'):
            flush()
            if code is None:
                code = []
            else:
                body.append('<pre><code>' + html.escape('\n'.join(code)) + '</code></pre>')
                code = None
            continue
        if code is not None:
            code.append(line)
            continue
        if not line.strip():
            flush()
            continue
        heading = re.fullmatch(r'(#{1,3}) (.+)', line)
        if heading:
            flush()
            level, label = len(heading[1]), heading[2]
            slug = re.sub(r'[^a-z0-9]+', '-', label.lower()).strip('-') or 'section'
            ids[slug] = ids.get(slug, 0) + 1
            anchor = slug if ids[slug] == 1 else f'{slug}-{ids[slug]}'
            if level == 1:
                title = label
            if level == 2:
                toc.append(f'<li><a href="#{anchor}">{html.escape(label)}</a></li>')
            body.append(f'<h{level} id="{anchor}">{inline(label)}</h{level}>')
            continue
        bullet = re.fullmatch(r'(- |\d+\. )(.+)', line)
        if bullet:
            kind = 'ul' if bullet[1] == '- ' else 'ol'
            if listing != kind:
                flush()
                listing = kind
                body.append('<' + kind + '>')
            body.append('<li>' + inline(bullet[2]) + '</li>')
        else:
            if listing:
                flush()
            paragraph.append(line.strip())
    if code is not None:
        raise ValueError('Unclosed code fence in user guide')
    flush()
    roles = {'bg': 'background', 'panel': 'panel', 'text': 'text', 'muted': 'muted',
             'accent': 'accent', 'border': 'divider'}

    def variables(palette):
        return ';'.join(f'--{name}:{palette[role]}' for name, role in roles.items())

    css = f''':root{{{variables(paper)};--space:8px;--radius:4px;color-scheme:light}}
@media(prefers-color-scheme:dark){{:root{{{variables(ink)};color-scheme:dark}}}}
*{{box-sizing:border-box}}html{{scroll-padding-top:24px}}
body{{margin:0;background:var(--bg);color:var(--text);font:17px/1.65 "Segoe UI",sans-serif}}
a{{color:var(--accent);text-underline-offset:3px}}a:focus-visible{{outline:3px solid var(--accent);outline-offset:4px}}
.mobile-toc{{display:none}}
.layout{{display:grid;grid-template-columns:240px minmax(0,1fr);max-width:1200px;margin:auto}}
nav{{padding:32px 24px;align-self:start;position:sticky;top:0;max-height:100vh;overflow:auto;border-right:1px solid var(--border)}}
nav h2{{font-size:17px;margin:0 0 16px}}nav ul{{list-style:none;margin:0;padding:0}}nav li{{margin:0 0 12px}}
main{{max-width:78ch;padding:40px 48px 80px;background:var(--panel);min-width:0}}
h1{{font-size:2.15rem;line-height:1.2;margin:0 0 24px;font-weight:650}}
h2{{font-size:1.45rem;line-height:1.3;margin:48px 0 16px;border-top:1px solid var(--border);padding-top:24px}}
h3{{font-size:1.1rem;margin:28px 0 8px}}p{{margin:0 0 20px}}li{{margin-bottom:10px}}ul,ol{{padding-left:24px}}
code{{font:0.9em Consolas,"Liberation Mono",monospace;overflow-wrap:anywhere}}pre{{padding:16px;border:1px solid var(--border);border-radius:var(--radius);white-space:pre-wrap;overflow-wrap:anywhere}}
.skip{{position:absolute;left:8px;top:-80px;padding:8px;background:var(--panel)}}.skip:focus{{top:8px}}
@media(max-width:900px){{.layout{{display:block}}.desktop-toc{{display:none}}.mobile-toc{{display:block;padding:16px 24px;border-bottom:1px solid var(--border)}}.mobile-toc summary{{font-weight:650;cursor:pointer}}nav{{position:static;max-height:none;border:0;border-bottom:1px solid var(--border)}}nav ul{{columns:2}}main{{padding:28px 24px;max-width:none}}}}
@media(max-width:450px){{nav ul{{columns:1}}main{{padding:24px 18px}}h1{{font-size:1.8rem}}}}
@media print{{body{{background:white;color:black;font-size:11pt}}nav,.skip{{display:none}}.layout{{display:block}}main{{padding:0;background:white;max-width:none}}h2{{break-after:avoid}}pre{{break-inside:avoid}}}}
'''
    return ('<!doctype html>\n<html lang="en"><head><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width,initial-scale=1">'
            '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src \'unsafe-inline\'; base-uri \'none\'; form-action \'none\'">'
            '<title>' + html.escape(title) + '</title><style>' + css + '</style></head>\n'
            '<body><a class="skip" href="#guide">Skip to guide</a><div class="layout">'
            '<nav class="desktop-toc" aria-label="Guide sections"><h2>In this guide</h2><ul>' + ''.join(toc) + '</ul></nav>'
            '<details class="mobile-toc"><summary>In this guide</summary><nav aria-label="Guide sections"><ul>' + ''.join(toc) + '</ul></nav></details>'
            '<main id="guide">' + '\n'.join(body) + '</main></div></body></html>\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Fail if the committed HTML differs; never write')
    args = parser.parse_args()
    output = ROOT / 'docs/USER-GUIDE.html'
    page = render((ROOT / 'docs/USER-GUIDE.md').read_text(encoding='utf-8'))
    if args.check:
        if not output.is_file() or output.read_text(encoding='utf-8') != page:
            parser.exit(1, 'Offline guide is stale; run python3 tools/build_docs.py\n')
        print('Offline guide matches its Markdown source and theme tokens')
    else:
        output.write_text(page, encoding='utf-8', newline='\n')
        print(output)


if __name__ == '__main__':
    main()
