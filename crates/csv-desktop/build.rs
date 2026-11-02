// SPDX-License-Identifier: Apache-2.0
use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=desktop.manifest");
    println!("cargo:rerun-if-changed=desktop.rc");
    let target = env::var("TARGET").unwrap();
    if !target.contains("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    if target.ends_with("gnu") {
        let obj = out.join("desktop.o");
        let status = Command::new("x86_64-w64-mingw32-windres")
            .args(["desktop.rc", "-O", "coff", "-o"])
            .arg(&obj)
            .status()
            .expect("windres required for Windows GUI manifest");
        assert!(status.success(), "Windows resource compilation failed");
        println!("cargo:rustc-link-arg-bin=csv-desktop={}", obj.display());
    } else {
        let obj = out.join("desktop.res");
        let status = Command::new("rc.exe")
            .arg("/nologo")
            .arg(format!("/fo{}", obj.display()))
            .arg("desktop.rc")
            .status()
            .expect("Windows SDK rc.exe required");
        assert!(status.success(), "Windows resource compilation failed");
        println!("cargo:rustc-link-arg-bin=csv-desktop={}", obj.display());
    }
}
