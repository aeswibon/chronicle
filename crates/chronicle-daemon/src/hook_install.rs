use anyhow::Context;
use std::fs;
use std::path::PathBuf;

const ZSH_HOOK: &str = include_str!("../../../assets/hooks/chronicle.zsh");
const FISH_HOOK: &str = include_str!("../../../assets/hooks/chronicle.fish");

pub fn install(shell: Option<&str>) -> anyhow::Result<()> {
    let home = dirs::home_dir().context("could not resolve home directory")?;
    let hooks_dir = home.join(".chronicle/hooks");
    fs::create_dir_all(&hooks_dir)?;

    fs::write(hooks_dir.join("chronicle.zsh"), ZSH_HOOK)?;
    fs::write(hooks_dir.join("chronicle.fish"), FISH_HOOK)?;

    let shell = shell
        .map(str::to_string)
        .or_else(detect_shell)
        .unwrap_or_else(|| "zsh".into());

    match shell.as_str() {
        "zsh" => install_zsh(&home, &hooks_dir)?,
        "fish" => install_fish(&home, &hooks_dir)?,
        other => anyhow::bail!("unsupported shell: {other} (use zsh or fish)"),
    }

    println!("Chronicle shell hook installed for {shell}");
    println!("Hooks directory: {}", hooks_dir.display());
    println!("Restart your shell or run: source {}", snippet_path(&shell, &home).display());
    Ok(())
}

pub fn print_hook(shell: &str) -> anyhow::Result<()> {
    match shell {
        "zsh" => print!("{ZSH_HOOK}"),
        "fish" => print!("{FISH_HOOK}"),
        other => anyhow::bail!("unsupported shell: {other}"),
    }
    Ok(())
}

fn detect_shell() -> Option<String> {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| {
            PathBuf::from(&s)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
}

fn install_zsh(home: &PathBuf, hooks_dir: &PathBuf) -> anyhow::Result<()> {
    let marker = "# chronicle shell hook";
    let line = format!(
        "{marker}\n[[ -f {}/chronicle.zsh ]] && source {}/chronicle.zsh\n",
        hooks_dir.display(),
        hooks_dir.display()
    );
    append_snippet(&home.join(".zshrc"), &marker, &line)
}

fn install_fish(home: &PathBuf, hooks_dir: &PathBuf) -> anyhow::Result<()> {
    let config_dir = home.join(".config/fish");
    fs::create_dir_all(&config_dir)?;
    let marker = "# chronicle shell hook";
    let line = format!(
        "{marker}\nsource {}/chronicle.fish\n",
        hooks_dir.display()
    );
    append_snippet(&config_dir.join("config.fish"), &marker, &line)
}

fn append_snippet(path: &PathBuf, marker: &str, block: &str) -> anyhow::Result<()> {
    if path.exists() {
        let existing = fs::read_to_string(path)?;
        if existing.contains(marker) {
            println!("Hook snippet already present in {}", path.display());
            return Ok(());
        }
        let mut content = existing;
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(block);
        fs::write(path, content)?;
    } else {
        fs::write(path, block)?;
    }
    Ok(())
}

fn snippet_path(shell: &str, home: &PathBuf) -> PathBuf {
    match shell {
        "fish" => home.join(".config/fish/config.fish"),
        _ => home.join(".zshrc"),
    }
}
