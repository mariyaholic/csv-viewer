# Repository tools

Owner: hermes.impl. No runtime dependency.

- `build_docs.py`: deterministic, offline user-guide export from the explicit Markdown subset; `--help`, `--check`; standard library only.
- `test_docs.py`: source/export, safety, control-copy and portable-packaging regressions; unittest discovery.

- `validate_design.py`: standard-library token/alias/contrast/state/runtime-parity checks; `--help`.
- `test_design.py`: adversarial validator regression tests; unittest discovery.
- `render_design.py`: illustrative component plates; `--help`; Pillow 9.4 via PEP 723 metadata. Uses Linux font fallbacks documented on the output.
