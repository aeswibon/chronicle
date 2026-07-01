fn main() {
    compile_macos_helpers();
}

fn compile_macos_helpers() {
    #[cfg(not(target_os = "macos"))]
    return;

    #[cfg(target_os = "macos")]
    {
        use std::env;
        use std::path::PathBuf;

        let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
        let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
        let scripts = manifest.join("scripts");

        let focus_bin = out.join("chronicle-focus-monitor");
        compile_swift(
            &focus_bin,
            &scripts.join("focus_monitor.swift"),
            &["AppKit", "ApplicationServices"],
        );
        println!(
            "cargo:rustc-env=CHRONICLE_FOCUS_MONITOR_HELPER={}",
            focus_bin.display()
        );

        let window_bin = out.join("chronicle-front-window");
        compile_swift(
            &window_bin,
            &scripts.join("front_window.swift"),
            &["CoreGraphics"],
        );
        println!(
            "cargo:rustc-env=CHRONICLE_FRONT_WINDOW_HELPER={}",
            window_bin.display()
        );
    }
}

#[cfg(target_os = "macos")]
fn compile_swift(out: &std::path::Path, script: &std::path::Path, frameworks: &[&str]) {
    use std::process::Command;
    let mut cmd = Command::new("swiftc");
    cmd.arg("-O").arg("-o").arg(out).arg(script);
    for fw in frameworks {
        cmd.arg("-framework").arg(*fw);
    }
    let ok = cmd.status().expect("failed to run swiftc").success();
    if !ok {
        panic!("swiftc failed to compile {}", script.display());
    }
}
