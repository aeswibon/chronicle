fn main() {
    compile_front_window_helper();
}

fn compile_front_window_helper() {
    #[cfg(not(target_os = "macos"))]
    return;

    #[cfg(target_os = "macos")]
    {
        use std::env;
        use std::path::PathBuf;
        use std::process::Command;

        let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
        let bin = out.join("chronicle-front-window");
        let script = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
            .join("scripts/front_window.swift");

        let ok = Command::new("swiftc")
            .arg("-O")
            .arg("-o")
            .arg(&bin)
            .arg(&script)
            .status()
            .expect("failed to run swiftc")
            .success();

        if !ok {
            panic!("swiftc failed to compile scripts/front_window.swift");
        }

        println!(
            "cargo:rustc-env=CHRONICLE_FRONT_WINDOW_HELPER={}",
            bin.display()
        );
    }
}
