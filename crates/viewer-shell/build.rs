// SPDX-License-Identifier: Apache-2.0
use std::{env, fs, path::PathBuf};

fn main() {
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../design/system/tokens.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let data: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let mut output = String::from("pub const BUTTON_MATERIALS: [ButtonMaterial; 6] = [\n");
    for id in ["paper", "ink", "pc98", "doloris", "sumimi", "marigold"] {
        let theme = &data["themes"][id];
        let refs = &theme["reference"];
        output.push_str("ButtonMaterial {");
        for (field, key) in [
            ("face", "control_face"),
            ("hot", "control_hover"),
            ("pressed", "control_pressed"),
            ("highlight", "control_highlight"),
            ("shadow", "control_shadow"),
            ("border", "border"),
            ("text", "text"),
            ("muted", "muted"),
            ("focus", "accent"),
        ] {
            let value = refs[key].as_str().expect("Missing button color");
            let rgb = u32::from_str_radix(value.strip_prefix('#').unwrap(), 16).unwrap();
            assert!(rgb <= 0xffffff);
            output.push_str(&format!("{field}:0x{rgb:06x},"));
        }
        let radius = theme["button_material"]["radius_dip"].as_u64().unwrap();
        let profile = match theme["button_material"]["profile"].as_str().unwrap() {
            "hard-bevel" => 1,
            "fine-frame" => 2,
            "warm-rim" => 3,
            "pressed-paper" | "machined" | "enamel" => 0,
            other => panic!("Unknown button material: {other}"),
        };
        let elevation = data["scale"]["button_elevation_dip"].as_u64().unwrap();
        let offset = data["scale"]["button_pressed_offset_dip"].as_u64().unwrap();
        assert!(radius <= 8 && elevation <= 4 && offset <= 2);
        output.push_str(&format!(
            "radius:{radius},profile:{profile},elevation:{elevation},offset:{offset}}},\n"
        ));
    }
    output.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("buttons.rs"),
        output,
    )
    .unwrap();
}
