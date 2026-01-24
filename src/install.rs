use crate::output::Output;
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub bin_dir: Option<PathBuf>,
    pub definitions_dir: Option<PathBuf>,
    pub definitions_src: Option<PathBuf>,
    pub skip_bin: bool,
    pub skip_definitions: bool,
}

pub fn run_install(opts: InstallOptions) -> Result<()> {
    let mut did_work = false;

    if !opts.skip_bin {
        install_binary(opts.bin_dir.as_deref())?;
        did_work = true;
    }

    if !opts.skip_definitions {
        install_definitions(opts.definitions_src.as_deref(), opts.definitions_dir.as_deref())?;
        did_work = true;
    }

    if !did_work {
        Output::warn("Nothing to install (both steps were skipped).");
    } else {
        Output::info("Completions: run `smart-command completions <shell>` to enable tab completion.");
    }

    Ok(())
}

fn install_binary(bin_dir_override: Option<&Path>) -> Result<()> {
    let exe = std::env::current_exe().context("Failed to resolve current executable path")?;
    let exe_name = exe
        .file_name()
        .ok_or_else(|| anyhow!("Failed to resolve executable file name"))?;

    let bin_dir = bin_dir_override
        .map(PathBuf::from)
        .unwrap_or_else(default_bin_dir);
    fs::create_dir_all(&bin_dir)
        .with_context(|| format!("Failed to create bin dir: {}", bin_dir.display()))?;

    let target = bin_dir.join(exe_name);
    if is_same_path(&exe, &target) {
        Output::info(&format!(
            "Binary already installed at {}",
            target.display()
        ));
    } else {
        fs::copy(&exe, &target).with_context(|| {
            format!(
                "Failed to copy binary from {} to {}",
                exe.display(),
                target.display()
            )
        })?;
        copy_permissions(&exe, &target).ok();
        Output::success(&format!("Installed binary to {}", target.display()));
    }

    if !path_contains_dir(&bin_dir) {
        Output::warn("Binary directory is not on PATH.");
        if cfg!(windows) {
            Output::dim(&format!(
                "Add this folder to PATH: {}",
                bin_dir.display()
            ));
        } else {
            Output::dim(&format!(
                "Add to your shell rc: export PATH=\"{}:$PATH\"",
                bin_dir.display()
            ));
        }
    }

    Ok(())
}

fn install_definitions(src_override: Option<&Path>, dest_override: Option<&Path>) -> Result<()> {
    let src = src_override
        .map(PathBuf::from)
        .or_else(find_definitions_source)
        .ok_or_else(|| {
            anyhow!(
                "Definitions source not found. Use --definitions-src to specify a path."
            )
        })?;

    let dest = dest_override
        .map(PathBuf::from)
        .unwrap_or_else(default_definitions_dir);

    if is_same_path(&src, &dest) {
        Output::info(&format!(
            "Definitions already installed at {}",
            dest.display()
        ));
        return Ok(());
    }

    let copied = copy_dir_recursive(&src, &dest)
        .with_context(|| format!("Failed to copy definitions from {}", src.display()))?;
    Output::success(&format!(
        "Installed {} definition files to {}",
        copied,
        dest.display()
    ));

    Ok(())
}

fn find_definitions_source() -> Option<PathBuf> {
    let candidates = [
        std::env::current_dir()
            .ok()
            .map(|p| p.join("definitions")),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("definitions"))),
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|p| p.exists() && p.is_dir())
}

fn default_definitions_dir() -> PathBuf {
    if let Some(dir) = dirs::config_dir() {
        return dir.join("smart-command").join("definitions");
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".config").join("smart-command").join("definitions");
    }
    PathBuf::from("definitions")
}

fn default_bin_dir() -> PathBuf {
    let home = dirs::home_dir();
    let local = dirs::data_local_dir();

    let mut candidates = Vec::new();
    if let Some(home) = &home {
        candidates.push(home.join(".cargo").join("bin"));
        candidates.push(home.join(".local").join("bin"));
        candidates.push(home.join("bin"));
    }
    if let Some(local) = &local {
        candidates.push(local.join("Programs").join("smart-command").join("bin"));
    }

    for dir in &candidates {
        if path_contains_dir(dir) {
            return dir.clone();
        }
    }

    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("."))
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<usize> {
    let mut copied = 0;
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copied += copy_dir_recursive(&path, &target)?;
        } else if path.is_file() {
            fs::copy(&path, &target)?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn copy_permissions(src: &Path, dest: &Path) -> std::io::Result<()> {
    let perms = fs::metadata(src)?.permissions();
    fs::set_permissions(dest, perms)
}

fn path_contains_dir(dir: &Path) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };

    let dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    std::env::split_paths(&path_var).any(|p| {
        fs::canonicalize(&p)
            .unwrap_or(p)
            .eq(&dir)
    })
}

fn is_same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if !a.exists() || !b.exists() {
        return false;
    }
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
