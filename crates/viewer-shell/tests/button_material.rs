// SPDX-License-Identifier: Apache-2.0
use viewer_shell::buttons::{material, visual, ButtonState};

#[test]
fn controls_have_generated_faces_and_distinct_native_states() {
    for i in 0..6 {
        let m = material(i);
        let rest = visual(m, ButtonState::Rest, false).unwrap();
        let hot = visual(m, ButtonState::Hot, false).unwrap();
        let down = visual(m, ButtonState::Pressed, false).unwrap();
        let disabled = visual(m, ButtonState::Disabled, false).unwrap();
        assert_ne!(rest.face, hot.face);
        assert_ne!(rest.face, down.face);
        assert_eq!(rest.elevation, 2);
        assert_eq!(down.elevation, 0);
        assert_eq!(down.offset, 1);
        assert_eq!(down.highlight, rest.shadow);
        assert_eq!(disabled.elevation, 0);
        assert_eq!(disabled.text, m.muted);
        assert!(visual(m, ButtonState::Hot, true).is_none());
    }
    assert_eq!(material(usize::MAX), material(0));
    assert_eq!(material(2).radius, 0);
}
