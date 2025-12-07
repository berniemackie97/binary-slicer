use std::env;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Canonicalize the root path if possible, falling back to the given string
/// relative to the current working directory.
pub fn canonicalize_or_current(root: &str) -> Result<PathBuf> {
    let path = Path::new(root);
    if path == Path::new(".") {
        Ok(env::current_dir().context("Failed to get current directory")?)
    } else {
        // Try to canonicalize; if it fails (e.g., path does not yet exist),
        // join it with the current dir to get an absolute path.
        match path.canonicalize() {
            Ok(p) => Ok(p),
            Err(_) => {
                let cwd = env::current_dir().context("Failed to get current directory")?;
                Ok(cwd.join(path))
            }
        }
    }
}

/// Infer a project name from the root path.
///
/// If the root has no final component (e.g., `/`), fallback to `unnamed-project`.
pub fn infer_project_name(root: &Path) -> String {
    root.file_name().and_then(|os_str| os_str.to_str()).unwrap_or("unnamed-project").to_string()
}

/// Compute the SHA-256 hash of a file and return it as a hex string.
pub fn sha256_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path)
        .with_context(|| format!("Failed to open binary for hashing: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = reader
            .read(&mut buf)
            .with_context(|| format!("Failed to read binary for hashing: {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    let digest = hasher.finalize();
    Ok(format!("{:x}", digest))
}
