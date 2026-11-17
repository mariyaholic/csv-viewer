#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Build or verify the allowlisted standalone MSVC ZIP. No publishing."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import struct
import zipfile

ROOT = Path(__file__).resolve().parents[1]
FILES = {
    "LICENSE": "LICENSE", "NOTICE": "NOTICE",
    "THIRD_PARTY.md": "THIRD_PARTY.md",
    "THIRD_PARTY_LICENSES.txt": "THIRD_PARTY_LICENSES.txt",
    "README.md": "docs/USER-GUIDE.md",
    "help/index.html": "docs/USER-GUIDE.html",
}
for font in ("Cinzel", "JetBrainsMono", "Outfit"):
    FILES[f"fonts/{font}.ttf"] = f"assets/desktop-fonts/{font}.ttf"
    FILES[f"fonts/{font}-OFL.txt"] = f"assets/desktop-fonts/{font}-OFL.txt"
BINARIES = ("csv-desktop.exe", "csv-worker.exe")
PAYLOAD = set(FILES) | set(BINARIES)
ARCHIVE_NAMES = PAYLOAD | {"BUILD.json", "SHA256SUMS"}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def check_pe(data):
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError("Expected a Windows executable")
    offset = struct.unpack_from("<I", data, 60)[0]
    if offset + 26 > len(data) or data[offset:offset + 4] != b"PE\0\0":
        raise ValueError("Missing PE header")
    if struct.unpack_from("<H", data, offset + 4)[0] != 0x8664:
        raise ValueError("Expected x86-64 machine")
    if struct.unpack_from("<H", data, offset + 24)[0] != 0x20B:
        raise ValueError("Expected PE32+ optional header")


def make_archive(entries, commit, profile):
    if set(entries) != PAYLOAD:
        raise ValueError("Payload differs from the exact standalone allowlist")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("A full lowercase source commit SHA is required")
    if profile not in ("debug", "release"):
        raise ValueError("Unknown build profile")
    for name in BINARIES:
        check_pe(entries[name])
    entries = dict(entries)
    entries["BUILD.json"] = (json.dumps({
        "source_commit": commit, "target": "x86_64-pc-windows-msvc",
        "profile": profile, "code_signed": False,
        "acceptance": "pending; see docs/RELEASE-CHECKLIST.md"
    }, sort_keys=True, indent=2) + "\n").encode()
    entries["SHA256SUMS"] = "".join(
        f"{digest(entries[name])}  {name}\n" for name in sorted(entries)
    ).encode()
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, data in sorted(entries.items()):
            # Fixed metadata: source paths, usernames and local mtimes are not copied.
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, data, compresslevel=9)
    result = output.getvalue()
    verify_archive(result)
    return result


def verify_archive(data):
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)) or set(names) != ARCHIVE_NAMES:
            raise ValueError("Archive differs from the exact standalone allowlist")
        entries = {name: archive.read(name) for name in names}
        expected = "".join(
            f"{digest(entries[name])}  {name}\n"
            for name in sorted(set(names) - {"SHA256SUMS"})
        ).encode()
        if entries["SHA256SUMS"] != expected:
            raise ValueError("Checksum manifest does not match archive contents")
        for name in BINARIES:
            check_pe(entries[name])
        info = json.loads(entries["BUILD.json"])
        if not re.fullmatch(r"[0-9a-f]{40}", info["source_commit"]):
            raise ValueError("Invalid source commit")
        if info["target"] != "x86_64-pc-windows-msvc" or info["profile"] not in ("debug", "release"):
            raise ValueError("Unexpected target/profile")
    return info


def read_regular(root, relative):
    root = root.resolve()
    path = root / relative
    # Refuse symlinks rather than accidentally packaging data outside the checkout.
    current = path
    while current != root:
        if current.is_symlink():
            raise ValueError(f"Symlink is not a release input: {relative}")
        current = current.parent
    if not path.is_file():
        raise ValueError(f"Missing release input: {relative}")
    return path.read_bytes()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary-dir", type=Path)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "dist")
    parser.add_argument("--commit")
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    parser.add_argument("--verify", type=Path)
    args = parser.parse_args()
    if args.verify:
        print(json.dumps(verify_archive(args.verify.read_bytes()), sort_keys=True))
        return
    if not args.binary_dir or not args.commit:
        parser.error("--binary-dir and --commit are required when building")
    entries = {name: read_regular(ROOT, path) for name, path in FILES.items()}
    entries.update({name: read_regular(args.binary_dir, name) for name in BINARIES})
    data = make_archive(entries, args.commit, args.profile)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    output = args.output_dir / f"csv-desktop-{args.profile}-windows-x64-msvc.zip"
    # Never silently overwrite an earlier candidate.
    checksum = output.with_suffix(output.suffix + ".sha256")
    if output.exists() or checksum.exists():
        raise ValueError("Candidate output already exists; choose a fresh output directory")
    with output.open("xb") as handle:
        handle.write(data)
    with checksum.open("x", encoding="utf-8", newline="\n") as handle:
        handle.write(f"{digest(data)}  {output.name}\n")
    print(output)


if __name__ == "__main__":
    main()
