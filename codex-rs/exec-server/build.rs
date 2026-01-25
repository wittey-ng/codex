use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        return;
    }

    if let Some(profile_dir) = profile_dir_from_out_dir()
        && let Some(runtime_dir) = discover_boxlite_runtime_dir(&profile_dir)
    {
        let target_runtime_dir = profile_dir.join("deps").join("runtime");
        if let Err(err) = sync_boxlite_runtime(&runtime_dir, &target_runtime_dir) {
            eprintln!(
                "warning: failed to stage BoxLite runtime from {} to {}: {err}",
                runtime_dir.display(),
                target_runtime_dir.display()
            );
        }
    }

    for bin in ["codex-execve-wrapper", "codex-exec-mcp-server"] {
        // Ensure @rpath resolves regardless of whether the binary lives in target/debug or
        // target/debug/deps.
        println!("cargo:rustc-link-arg-bin={bin}=-Wl,-rpath,@executable_path/deps/runtime");
        println!("cargo:rustc-link-arg-bin={bin}=-Wl,-rpath,@executable_path/../deps/runtime");
    }
}

fn profile_dir_from_out_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").ok()?);
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}

fn discover_boxlite_runtime_dir(profile_dir: &Path) -> Option<PathBuf> {
    let build_dir = profile_dir.join("build");
    let entries = fs::read_dir(&build_dir).ok()?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if !name.starts_with("boxlite-") {
            continue;
        }
        let runtime_dir = entry.path().join("out").join("runtime");
        if runtime_dir.join("mke2fs").is_file() {
            return Some(runtime_dir);
        }
    }
    None
}

fn sync_boxlite_runtime(source_dir: &Path, target_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target_dir)?;
    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let source_path = entry.path();
        let target_path = target_dir.join(&file_name);
        if target_path.exists() {
            continue;
        }
        fs::copy(&source_path, &target_path)?;
        #[cfg(unix)]
        {
            let perms = fs::metadata(&source_path)?.permissions();
            fs::set_permissions(&target_path, perms)?;
        }
    }
    Ok(())
}
