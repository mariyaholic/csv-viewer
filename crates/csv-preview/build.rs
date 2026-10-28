// SPDX-License-Identifier: Apache-2.0

fn main() {
    // Allows a non-Windows host to type-check the Windows-only module:
    //   RUSTFLAGS='--cfg csvview_windows_check' cargo check -p csv-preview
    println!("cargo:rustc-check-cfg=cfg(csvview_windows_check)");
}
