# Changelog

All notable changes are recorded here in the style of Keep a Changelog.

## [0.1.0] - Unreleased

- Add a read-only CSV viewer, parser, isolated worker and Windows preview handler.
- Remove empty registry keys bottom-up on unregister while preserving foreign content.
- Show scan coverage and source-text warnings; support explicit full scans.

- Copy Explorer streams on the COM caller before handing a bounded file to the spool thread.
- Roll back successful registry value writes when registration fails, when values remain unchanged.
- Size the preview text from the client area and font metrics; scale the font with DPI.
- Audit dependency licences and retain Unicode and encoding data notices.
