# SPDX-License-Identifier: Apache-2.0
"""Adversarial contract tests; run with unittest discovery in tools/."""
import copy
import json
import unittest
from pathlib import Path

from validate_design import validate, check_runtime

ROOT = Path(__file__).resolve().parents[1]


class DesignContractTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads((ROOT / 'design/system/tokens.json').read_text())

    def test_real_contract_and_all_six_themes(self):
        result = validate(self.data)
        self.assertEqual(set(result), {'paper', 'ink', 'pc98', 'doloris', 'sumimi', 'marigold'})
        self.assertTrue(all(len(pairs) >= 20 for pairs in result.values()))

    def test_current_runtime_palette_is_preserved(self):
        check_runtime(self.data, ROOT / 'crates/viewer-shell/src/lib.rs')

    def test_missing_component_binding_is_rejected(self):
        del self.data['components']['grid.text']
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_invalid_spacing_scale_is_rejected(self):
        self.data['scale']['spacing_dip'] = [4, 8, -1, 8]
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_missing_type_face_is_rejected(self):
        self.data['themes']['paper']['typography']['data'] = ''
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_reduced_motion_must_disable_transitions(self):
        self.data['scale']['motion_ms']['reduced'] = 180
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_specimen_has_no_taglines_or_instructional_filler(self):
        source = (ROOT / 'tools/render_design.py').read_text()
        for phrase in ('The data workbench', 'A different material finish',
                       'Future command anatomy', 'Scope is never implied by color',
                       'Leading zeros stay intact', 'Copy action names its representation',
                       'Interaction grammar', 'Keep entered value; explain recovery'):
            self.assertNotIn(phrase, source)

    def test_button_elevation_tokens_are_required(self):
        self.data['semantic'].pop('control.highlight', None)
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_missing_theme_is_rejected(self):
        del self.data['themes']['pc98']
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_missing_role_is_rejected(self):
        del self.data['semantic']['focus.ring']
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_alias_cycle_is_rejected(self):
        self.data['semantic']['text.primary'] = '{semantic.text.primary}'
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_missing_alias_is_rejected(self):
        self.data['components']['grid.text'] = '{semantic.not-here}'
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_component_cannot_bypass_roles(self):
        self.data['components']['grid.text'] = '{reference.text}'
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_case_collision_is_rejected(self):
        self.data['semantic']['Text.Primary'] = '{reference.text}'
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_status_text_contrast_is_checked(self):
        self.data['themes']['doloris']['reference']['error'] = '#0b0913'
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_control_boundary_contrast_is_checked(self):
        self.data['themes']['paper']['reference']['border'] = '#fffdf8'
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_stale_and_cancelled_states_are_required(self):
        self.data['states'].remove('stale')
        with self.assertRaises(ValueError):
            validate(self.data)

    def test_runtime_palette_drift_is_rejected(self):
        changed = copy.deepcopy(self.data)
        changed['themes']['paper']['reference']['background'] = '#ffffff'
        with self.assertRaises(ValueError):
            check_runtime(changed, ROOT / 'crates/viewer-shell/src/lib.rs')


if __name__ == '__main__':
    unittest.main()
