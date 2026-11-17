Made by Maria

# CSV viewer and preview

Hello! This is my CSV viewer. It inspects a CSV in Explorer or a standalone
window, and it always gives you an explicit account of parsing and scan
coverage. I like that it never pretends to know more than it read, and I hope
it helps you look at your files as much as it helps me.

This is free and open source (Apache-2.0). See [LICENSE](LICENSE) and [NOTICE](NOTICE).

Windows binaries are not code-signed yet; Windows SmartScreen may warn when they are run.

MT5/MetaTrader integration is not included. CSV viewer and preview is not
affiliated with MetaQuotes; MT5 is a MetaQuotes trademark.

## Everyday use

- In the app, choose **Help** for topics or the bundled offline guide; **F1** opens Quick help.
- Ask for the full scan when you want the whole file covered: F in the preview window, f in the viewer. A partial scan tells you exactly what it read, down to the bytes and the reason it stopped. It never guesses a total.
- Review notes are notes, not conversions. Leading zeros, ambiguous dates, ragged rows and other oddities get flagged with samples and counts, and the source cells stay unchanged.

There is no public release yet. The first planned public beta is the standalone
Windows x64 desktop app, and I will announce it here when it ships. Until then,
my [documentation page](https://mariyaholic.github.io/docs/csv-viewer/) has the
build steps, in case you want to try it early.

## Everything else

The technical detail lives on [the documentation page](https://mariyaholic.github.io/docs/csv-viewer/) on my site:

- [Use the desktop app](https://mariyaholic.github.io/docs/csv-viewer/#desktop) — the portable ZIP and the six themes
- [Use the Explorer preview](https://mariyaholic.github.io/docs/csv-viewer/#explorer) — the opt-in registration and what it keeps when it cleans up
- [Detect the encoding and dialect](https://mariyaholic.github.io/docs/csv-viewer/#formats) — four encodings, four delimiters and the overrides
- [Use the terminal viewer](https://mariyaholic.github.io/docs/csv-viewer/#terminal) — the commands and the keys
- [The settings file](https://mariyaholic.github.io/docs/csv-viewer/#settings) — where it lives and what it holds
- [Build from source](https://mariyaholic.github.io/docs/csv-viewer/#build) and [build for Windows](https://mariyaholic.github.io/docs/csv-viewer/#windows-builds)
- [Limits](https://mariyaholic.github.io/docs/csv-viewer/#limits) — every one stated up front
- [Market heuristics](https://mariyaholic.github.io/docs/csv-viewer/#market) — optional, read-only, and not financial advice
- [Release status](https://mariyaholic.github.io/docs/csv-viewer/#status) — the beta plan and the checklist I keep
- [Start here: desktop user guide](https://github.com/mariyaholic/csv-viewer/blob/main/docs/USER-GUIDE.md) — opening, browsing, searching, copying and appearance

Thank you for reading. ♡
