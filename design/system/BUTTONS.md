# Button contract — 0.2.0

Applies to all six design-system specimens and the native button adapter. The
apps now use generated materials on standard Button HWNDs via NM_CUSTOMDRAW.
Menu/split examples remain specimens; no unsupported commands were added.

## Conventions adopted

- Use concise action labels in sentence case; add the object when needed.[1][2]
  Contract examples: `Reload`, `Copy shown text`, `Open file…`.
- Keep ordinary workbench commands neutral. At most one prominent primary action
  per layout; equal-priority commands may all be neutral.[1] The specimens show
  one primary example in the state gallery, not multiple competing gold commands.
- An ellipsis means additional input is required to complete the command, not
  merely that another window opens.[3] `Open file…` chooses a file; `Reload` and
  `Copy shown text` act immediately. `About` has no ellipsis.
- `Export` with a chevron opens a menu. A subsequent `Export sample…` command
  would request a destination. Do not use an ellipsis as a menu indicator.
  A split button exposes a default action separately from its menu.[1][2]
- Center text labels and use consistent height/padding; reserve indicator space.
  Current Windows button guidance recommends a 120px minimum width for shorter
  command labels; this specimen uses a 120 logical-pixel minimum, not a universal
  claim about every native toolbar control.[2] Measure localized text, never clip.
- Support keyboard activation and appropriate focus transfer/return using native
  controls and UIA. W3C describes Space/Enter activation and focus behavior for
  web buttons; its ARIA attributes are not native Win32 implementation APIs.[4]
- Separate primary emphasis, keyboard default, focus and persistent toggle state.
  The gallery's `Rest` example is an ordinary resting appearance, not an Enter-key
  default. A split button is also not a toggle.[1][3][4]

## Material choices

These are product choices, not requirements from the cited systems. Surfaces and
edges read shared semantic tokens, never theme color literals in a component.

- Paper: warm raised face, fine rounded boundary and short paper-toned shadow.
- Ink: cool dark raised face, restrained highlight and tight machined corners.
- PC-98: navy face, square two-sided hard bevel and crisp contact edge.
- Doloris: plum raised face with a fine inner frame; gold stays an emphasis/focus cue.
- Sumimi: ivory enamel face, softened corners, light upper edge and warm lower lip.
- Marigold: warm plum face and doubled warm rim, without turning ordinary actions orange.

Rest has 2 DIP elevation; pressed loses its shadow, reverses the lighting and
moves content 1 DIP. Hover changes the face without geometry movement. Focus uses
an independent 2 DIP ring separated by 2 DIP. Disabled loses elevation and retains
legible muted text. Busy names the ongoing operation and prevents duplicate
activation in a future implementation; Cancel stays actionable. No flashing or
motion is required. OS high contrast overrides authored lighting and palette.

Normal text, disabled labels as an intentional product floor, and action labels
are tested at 4.5:1 on their relevant faces. Focus is tested at 3:1. Decorative
highlights and contact shadows are not interactive boundaries; the existing
control boundary remains separate. Fluent exempts disabled buttons from contrast
requirements; our stricter muted-label check is a product decision.[1]

## Acceptance

All six themes must render neutral, primary, rest, hover, pressed, focused,
disabled, busy, menu and split examples without clipped labels. Native adoption
also needs keyboard/default-command safety, screen-reader names/state, forced
colors, DPI/text scaling and enlarged/localized labels. A rendered plate cannot
prove those behaviors. The Win32 source below is explicitly legacy Windows 7
UX guidance; use its precise ellipsis/default-command distinctions, not its old
styling as a current Windows visual specification.[3]

## Sources

[1] https://fluent2.microsoft.design/components/web/react/core/button/usage
[2] https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/buttons
[3] https://learn.microsoft.com/en-us/windows/win32/uxguide/ctrl-command-buttons
[4] https://www.w3.org/WAI/ARIA/apg/patterns/button