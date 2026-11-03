# Viewer design system — six faces, one workbench

Version 0.1.0. Design contract for CSV and Parquet desktop companions, not a claim
that the future controls shown here already exist in the native applications.

## Direction card

- **Audience:** people inspecting unfamiliar local data, often under time pressure.
- **Concept:** a precision reading bench with six interchangeable material finishes.
- **Three words:** legible, measured, tactile.
- **References:** MH Ticker's Paper/PC-98 faces; Uika canonical Doloris/Sumimi tokens
  (`Uika/design/uika-canonical.css`, ADR-0011). Marigold is a derived face, not a new canon.
- **Density:** data-dense, calm chrome; controls and coverage remain readable.
- **Forbidden:** dashboard cards around every value, ornamental data cells,
  color-only truth claims, six component implementations, decorative charts.

## Architecture

`reference values → semantic roles → component bindings → native control adapter`.

`tokens.json` is a versioned JSON contract with explicit aliases, not a claim of
full DTCG compliance or a W3C standard. Its baseline six palette roles are
copied from the current Rust theme catalog; extended status, input, selection,
border and interaction roles are proposed additions. Canonical Uika colors keep
their meaning. Lighter danger text on dark panels and darker success text on ivory are documented
functional contrast adaptations, not a replacement for Uika's lining primitive.

Canonical design source: `qol-portfolio/design/viewer-system/`. Matching portable
snapshots live at `design/system/` in both viewer repositories. Update all three
in one change; the validation tool accepts another token file for parity checks.
No runtime Rust source or app behavior is changed by this design-system package.
A later implementation should generate native color bindings from this contract,
then retire duplicated literals only after parity tests and native acceptance.

## Foundations

- Layout units are **DIPs at 96 DPI**, never unscaled physical pixels. Scale once
  at the native boundary. Body/data type defaults to 12 pt (16 logical px).
- Spacing: 4, 8, 12, 16, 24, 32. Normal row height 32; compact 26; comfortable 38.
  Density is a separate user setting, not a side effect of switching themes.
- Control height 32; minimum actionable target 28 × 28; standard dialog/OS controls
  may be larger. Keyboard focus remains available for every command.
- UI type 12 pt, data 12 pt, caption 10.5 pt, section 14 pt, document title 18 pt.
  Cinzel is for headings only. Never use inscription type for cells or long errors.
- Data cells are single-line previews. Inspectors carry wrapping values and copy
  labels that say exactly what is copied. Numeric alignment is right; identifiers,
  timestamps and raw source text stay left unless their semantic type is proven.
- Primary text, muted text, status text and action labels must reach 4.5:1.
  Focus rings, essential boundaries and selected outlines must reach 3:1.
  Decorative dividers are exempt only if not needed to identify a control.
- Motion: none in the grid; optional 120 ms UI feedback, 180 ms panel transitions.
  Reduced motion disables both. No pulsing loading skeleton or animated ticker.
- Shadows are not required. Elevation is indicated by panel roles, a boundary and
  focus ownership; native menus/dialogs retain OS chrome.

## Six identities without six layouts

- **Paper:** warm reading paper, restrained ochre commands, utilitarian UI title,
  flat fine rules. The everyday CSV-reading face.
- **Ink:** deep neutral-plum substrate, silver UI typography, restrained lavender
  annotation, square instrument edges. Gold reserved for the active command/focus.
- **PC-98:** navy instrument substrate, MS Gothic-style UI/data typography,
  hard rectangular outlines and stepped, nonanimated status marks. No bitmap
  font inside dense values if it costs glyph coverage or legibility.
- **Doloris:** deepest plum, gold hierarchy and lavender information, Cinzel
  section headings, formal fine-rule framing. Platinum values stay unornamented.
- **Sumimi:** ivory substrate with inscription headings, editorial gold rules
  and lavender annotations. Distinct from Paper through type, accent placement
  and edge treatment, not gratuitous rearrangement of the same toolbar.
- **Marigold:** raised plum ground, orange **ornamental trim**, Cinzel headings,
  stronger section framing. Orange never becomes an implicit error or warning.

UI copy is functional only: labels, values, scope, errors and recovery actions.
No taglines, design commentary or explanatory filler in application surfaces.

All themes keep the same command order, controls, shortcuts, status language and
information architecture. Native buttons, menus, selection and high-contrast
colors may differ from the illustrative component plates. Theme switching must
not reset a file, selection, search, scroll offset, density or an active operation.

## Use when adding a feature

1. Choose a recipe in `FEATURE-RECIPES.md`; name its data scope and trust level.
2. Reuse a component in `COMPONENTS.md`; add a shared component only if none fits.
3. Bind semantic roles; never put a theme-specific color literal in a component.
4. Define empty, loading, loaded, partial, failed, cancelled and stale states.
5. Define keyboard focus, announcements, error recovery and worker cost.
6. Run the token validator and adversarial tests, then implement native controls.
7. Exercise the feature in all six themes, all three densities, high contrast, reduced
   motion, 100/150/200% DPI, keyboard-only and native Narrator/UIA.

## Validation and limits

From either viewer repository:

```sh
python3 tools/validate_design.py
python3 -m unittest discover -s tools -p 'test_design.py'
python3 tools/validate_design.py --peer /path/to/other/design/system/tokens.json
```

Token contrast validation is not a rendered accessibility certification. Native
forced-color, text scaling, focus visibility and UIA reading remain bench tests.
`components-*.png` and `six-system-faces.png` are **illustrative future-component
specimens**, not working application screenshots. They use documented fallback
fonts where target Windows faces are unavailable: DejaVu Sans/Mono for Segoe UI/Consolas,
and Unifont for MS Gothic. Uika plates use the bundled Cinzel/Outfit/JetBrains Mono.
