# SPDX-License-Identifier: Apache-2.0
"""Pure in-memory packaging regressions; no Windows toolchain required."""
import importlib.util
import io
import json
from pathlib import Path
import struct
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("package_desktop", ROOT / "scripts/package-desktop.py")
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class PackageTests(unittest.TestCase):
    def entries(self):
        pe = bytearray(128)
        pe[:2] = b"MZ"
        struct.pack_into("<I", pe, 60, 64)
        pe[64:68] = b"PE\0\0"
        struct.pack_into("<H", pe, 68, 0x8664)
        struct.pack_into("<H", pe, 88, 0x20B)
        entries = {name: f"synthetic {name}\n".encode() for name in package.FILES}
        entries.update({name: bytes(pe) for name in package.BINARIES})
        return entries

    def archive(self, entries=None):
        return package.make_archive(self.entries() if entries is None else entries, "a" * 40, "release")

    def rewrite(self, data, change):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            entries = {name: archive.read(name) for name in archive.namelist()}
        change(entries)
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w") as archive:
            for name, value in entries.items():
                archive.writestr(name, value)
        return output.getvalue()

    def test_exact_payload_and_all_checksums(self):
        data = self.archive()
        self.assertEqual(package.verify_archive(data)["source_commit"], "a" * 40)
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            self.assertEqual(set(archive.namelist()), package.ARCHIVE_NAMES)
            self.assertNotIn("csv_preview.dll", archive.namelist())
            self.assertNotIn("csv-viewer.exe", archive.namelist())

    def test_archive_is_deterministic(self):
        self.assertEqual(self.archive(), self.archive())
        with zipfile.ZipFile(io.BytesIO(self.archive())) as archive:
            self.assertTrue(all(x.date_time == (1980, 1, 1, 0, 0, 0) for x in archive.infolist()))

    def test_every_missing_input_fails(self):
        for name in package.PAYLOAD:
            with self.subTest(name=name):
                entries = self.entries()
                del entries[name]
                with self.assertRaises(ValueError):
                    self.archive(entries)

    def test_unexpected_private_file_fails(self):
        entries = self.entries()
        entries["settings.json"] = b"private"
        with self.assertRaises(ValueError):
            self.archive(entries)

    def test_tampered_payload_and_manifest_fail(self):
        for name in ("help/index.html", "SHA256SUMS"):
            with self.subTest(name=name):
                data = self.rewrite(self.archive(), lambda entries: entries.update({name: b"tampered"}))
                with self.assertRaises(ValueError):
                    package.verify_archive(data)

    def test_extra_or_missing_archive_member_fails(self):
        for change in (lambda entries: entries.update({"../private.txt": b"x"}),
                       lambda entries: entries.pop("fonts/Outfit-OFL.txt")):
            with self.assertRaises(ValueError):
                package.verify_archive(self.rewrite(self.archive(), change))

    def test_wrong_platform_and_short_binaries_fail(self):
        for payload in (b"ELF", b"MZ", bytes(128)):
            entries = self.entries()
            entries["csv-desktop.exe"] = payload
            with self.assertRaises(ValueError):
                self.archive(entries)
        entries = self.entries()
        data = bytearray(entries["csv-worker.exe"])
        struct.pack_into("<H", data, 68, 0x14C)
        entries["csv-worker.exe"] = bytes(data)
        with self.assertRaises(ValueError):
            self.archive(entries)

    def test_duplicate_archive_member_fails(self):
        original = self.archive()
        buffer = io.BytesIO(original)
        with zipfile.ZipFile(buffer, "a") as archive:
            import warnings
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", UserWarning)
                archive.writestr("README.md", b"duplicate")
        with self.assertRaises(ValueError):
            package.verify_archive(buffer.getvalue())

    def test_symlink_or_missing_inputs_fail_without_reading(self):
        for leaf in ("fonts", "Outfit.ttf"):
            with self.subTest(leaf=leaf):
                with patch.object(Path, "resolve", return_value=Path("/virtual")), \
                     patch.object(Path, "is_symlink", lambda p: p.name == leaf), \
                     patch.object(Path, "read_bytes") as reader:
                    with self.assertRaises(ValueError):
                        package.read_regular(Path("/virtual"), "fonts/Outfit.ttf")
                    reader.assert_not_called()
        with patch.object(Path, "resolve", return_value=Path("/virtual")), \
             patch.object(Path, "is_symlink", return_value=False), \
             patch.object(Path, "is_file", return_value=False), \
             patch.object(Path, "read_bytes") as reader:
            with self.assertRaises(ValueError):
                package.read_regular(Path("/virtual"), "LICENSE")
            reader.assert_not_called()

    def test_source_pin_and_profile_required(self):
        for commit, profile in (("main", "release"), ("a" * 40, "production")):
            with self.assertRaises(ValueError):
                package.make_archive(self.entries(), commit, profile)

    def test_debug_is_clearly_labelled(self):
        data = package.make_archive(self.entries(), "a" * 40, "debug")
        self.assertEqual(package.verify_archive(data)["profile"], "debug")


if __name__ == "__main__":
    unittest.main()
