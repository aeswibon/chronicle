fn main() {
    tauri_build::build();
    compile_icon_helper();
}

fn compile_icon_helper() {
    #[cfg(not(target_os = "macos"))]
    return;

    #[cfg(target_os = "macos")]
    {
        use std::env;
        use std::path::PathBuf;
        use std::process::Command;

        let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
        let bin = out.join("chronicle-icon");
        let script = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
            .join("scripts/app_icon.swift");

        let ok = Command::new("swiftc")
            .arg("-O")
            .arg("-o")
            .arg(&bin)
            .arg(&script)
            .status()
            .expect("failed to run swiftc")
            .success();

        if !ok {
            panic!("swiftc failed to compile scripts/app_icon.swift");
        }

        println!("cargo:rustc-env=CHRONICLE_ICON_HELPER={}", bin.display());
    }
}
