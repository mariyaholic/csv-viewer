# SPDX-License-Identifier: Apache-2.0
"""Offline documentation regression checks; python -m unittest discover -s tools -p test_docs.py."""
import importlib.util
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location("build_docs", ROOT/"tools/build_docs.py")
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
class DocsTests(unittest.TestCase):
    def test_html_is_semantic_offline_and_escaped(self):
        page=module.render("# Example\n\n## Open a file\n\nUse `file.csv` and **Open file**.\n\n- One\n- Two\n\n1. Start\n2. Close\n\n```text\n<script>bad</script>\n```\n")
        for expected in ['lang="en"','<main','<nav','<h1','<h2','<ul>','<ol>','<code>file.csv</code>','&lt;script&gt;','Content-Security-Policy']:
            self.assertIn(expected,page)
        self.assertNotIn('<script>',page)
        self.assertNotIn('https://',page)
        self.assertNotIn('<iframe',page)
    def test_unclosed_code_is_not_silently_lost(self):
        with self.assertRaises(ValueError):module.render('# Example\n```text\nlost')
    def test_ids_stay_unique_for_repeated_headings(self):
        page=module.render('# Example\n## Open\nA\n## Open\nB')
        self.assertIn('id="open"',page)
        self.assertIn('id="open-2"',page)
    def test_checked_in_guide_matches_source(self):
        self.assertEqual((ROOT/'docs/USER-GUIDE.html').read_text(encoding='utf-8'),module.render((ROOT/'docs/USER-GUIDE.md').read_text(encoding='utf-8')))
    def test_guide_covers_actual_controls_and_safety(self):
        source=(ROOT/'docs/USER-GUIDE.md').read_text(encoding='utf-8')
        for word in ['Open file','Clear search','Next','Copy shown row','F1','64 columns','200 rows','metadata','read-only','normal close','4 GiB','16 GiB','64 MiB','unsigned']:
            self.assertIn(word,source)
    def test_packaging_keeps_offline_guide_and_hashes_it(self):
        source=(ROOT/'scripts/build-desktop.sh').read_text(encoding='utf-8')
        self.assertIn('help/index.html',source)
        self.assertIn('python3 tools/build_docs.py --check',source)
        self.assertIn('help/*',source)
if __name__=='__main__':unittest.main()
