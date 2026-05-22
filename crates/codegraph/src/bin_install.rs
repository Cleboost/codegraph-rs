use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::path::Path;

pub fn ensure_installed() -> Result<Utf8PathBuf> {
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;
    let exe_name = current_exe
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("codegraph"));
    
    let home_dir = dirs::home_dir().context("Could not find home directory")?;
    let local_bin = home_dir.join(".local").join("bin");
    
    if !local_bin.exists() {
        std::fs::create_dir_all(&local_bin).context("Failed to create ~/.local/bin directory")?;
    }
    
    let target_exe = local_bin.join(exe_name);
    
    // Check if we are already running from the target location
    if current_exe == target_exe {
        return Utf8PathBuf::from_path_buf(target_exe)
            .map_err(|p| anyhow::anyhow!("non-UTF8 target exe path: {}", p.display()));
    }
    
    // If target exists, try to remove it first to avoid text file busy errors
    if target_exe.exists() {
        let _ = std::fs::remove_file(&target_exe);
    }
    
    // Copy ourselves to the target location
    std::fs::copy(&current_exe, &target_exe).context("Failed to copy binary to ~/.local/bin")?;
    
    // Add to PATH
    ensure_in_path(&local_bin)?;
    
    Utf8PathBuf::from_path_buf(target_exe)
        .map_err(|p| anyhow::anyhow!("non-UTF8 target exe path: {}", p.display()))
}

#[cfg(windows)]
fn ensure_in_path(bin_dir: &Path) -> Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;
    
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
    let current_path: String = env_key.get_value("Path").unwrap_or_default();
    
    let bin_str = bin_dir.to_string_lossy().to_string();
    
    if !current_path.contains(&bin_str) {
        let new_path = if current_path.is_empty() {
            bin_str
        } else {
            format!("{};{}", current_path, bin_str)
        };
        env_key.set_value("Path", &new_path)?;
        eprintln!("Added {} to User PATH. Please restart your terminal.", bin_dir.display());
    }
    
    Ok(())
}

#[cfg(not(windows))]
fn ensure_in_path(bin_dir: &Path) -> Result<()> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    let profiles = [".bashrc", ".zshrc", ".profile"];
    let bin_str = bin_dir.to_string_lossy().to_string();
    let export_line = format!("export PATH=\"{}:$PATH\"", bin_str);
    
    let mut added = false;
    for p in profiles {
        let profile_path = home.join(p);
        if profile_path.exists() {
            let content = std::fs::read_to_string(&profile_path).unwrap_or_default();
            if !content.contains(&export_line) {
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(&profile_path) {
                    let _ = file.write_all(format!("\n{}\n", export_line).as_bytes());
                    added = true;
                }
            }
        }
    }
    if added {
        eprintln!("Added {} to PATH in your shell profile. Please restart your terminal.", bin_dir.display());
    }
    Ok(())
}
