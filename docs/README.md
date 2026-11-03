# Documentation

## Using the desktop app

- [User guide](USER-GUIDE.md): start here. Open a file, browse, search, inspect/copy rows, remember appearance and fix a problem.
- [Standalone usage](STANDALONE-USAGE.md): short launch instructions and links for existing readers.
- Offline help: extract the complete portable ZIP, then choose Help → Offline user guide. F1 opens Quick help without a browser.

The user guide is included as `help/index.html` with no network resources or scripts.
The desktop app is not the terminal viewer or Explorer preview; their commands and limits differ.

## Building, testing and support

- [Desktop development and Windows acceptance](DEVELOPMENT.md): builds, native smoke host, portable payload and Maria's pending local checklist.
- [Large-file browsing](LARGE-FILE-BROWSING.md): exact search scope, format limits and worker behavior.
- [Standalone architecture](STANDALONE-ARCHITECTURE.md): shared shell and desktop adapter boundaries.
- [Core/preview architecture](ARCHITECTURE.md): backend and Explorer integration.
- [Documentation maintenance](DOCUMENTATION.md): voice, source-of-truth map and offline export checks.
- Repository status: [STATUS.md](../STATUS.md). Deferred work: [ROADMAP.md](../ROADMAP.md).

Windows Explorer, default-app chooser, Narrator/UIA and real display acceptance remain owner checks, not implications of passing CI.

## Runtime state and debugging

- [State-machine inventory, invariants and regression coverage](STATE-MACHINES.md)
