# Documentation maintenance

## Reader and reading order

The primary reader has extracted a portable viewer and wants to inspect data without learning Rust, COM or a terminal.
Order: launch → open → browse/search → inspect/copy → appearance → help → recovery → limits/privacy → reporting.
Use prose and numbered actions; source-format limits stay explicit. The developer track is separate in DEVELOPMENT.md.

## Voice

Traits: direct, calm, specific, non-promotional. Tone is practical in guides and calm/actionable after errors.
Rules: use the visible control name; say what the action changes; put the next safe action beside a limitation.
Do: Open, Choose, Retry. Avoid: seamless, powerful, simply.
Examples from the UI: “Open file…”, “Clear search”, “Select a row for details.”,
“No matches in the rows checked. Choose Next to keep searching.”, “Appearance not saved”.
No taglines, reading-level claims or unsupported accessibility/performance claims.

## Sources of truth

- USER-GUIDE.md is the human-edited user guide. USER-GUIDE.html is a committed generated artifact.
- tools/build_docs.py uses a documented small Markdown subset and only the Python standard library; no renderer/toolchain install is required. It escapes text and generates no scripts or external resources.
- The HTML reading surface inherits Paper/Ink colors from design/system/tokens.json. It follows browser color preference, not the viewer's selected theme. The layout is a reference manual with a section rail, not a landing page.
- Native task help and keyboard/control labels live in crates/viewer-shell/src/guidance.rs and win32.rs.
- Limits are backed by LARGE-FILE-BROWSING.md and the browse adapters/protocol. Appearance behavior is backed by preferences.rs and its tests.
- Claims about complete search, original copied values, full-file validation or native accessibility must not be inferred from a displayed page.
- Developer history/fix plans are retained as history; do not route new users through them.

## Export and checks

```sh
python3 tools/build_docs.py
python3 tools/build_docs.py --check
python3 -m unittest discover -s tools -p 'test_docs.py'
```

The supported Markdown subset is headings through level three, paragraphs, flat bullet/number lists,
fenced code, inline code and bold. Keep the user guide within it; raw HTML is escaped, not executed.
The portable build checks the committed export before copying it to help/index.html and checksumming it.
CI checks source/export parity. Native smoke keeps real F1/Help/content/recovery assertions.
Semantic HTML, a skip link, focus styling and print styles are implemented; no accessibility certification is claimed.
