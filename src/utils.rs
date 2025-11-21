use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::Result;
use encoding_rs::WINDOWS_1252;
use sha2::{Digest, Sha256};

pub fn parse_size(s: &str) -> u64 {
    let s = s.trim().to_lowercase();
    let units = [
        ("gib", 1024u64.pow(3)),
        ("g", 1000u64.pow(3)),
        ("mib", 1024u64.pow(2)),
        ("m", 1000u64.pow(2)),
        ("kib", 1024),
        ("k", 1000),
        ("kb", 1000),
        ("mb", 1000u64.pow(2)),
        ("gb", 1000u64.pow(3)),
        ("b", 1),
    ];

    for (unit, mult) in units {
        if s.ends_with(unit) {
            if let Ok(val) = s.trim_end_matches(unit).parse::<f64>() {
                return (val * mult as f64) as u64;
            }
        }
    }
    s.parse().unwrap_or(0)
}

pub fn is_probably_binary(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return true,
    };
    let mut buffer = [0u8; 4096];
    let n = match file.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return true,
    };
    if n == 0 {
        return false;
    }

    let slice = &buffer[..n];
    if slice.contains(&0) {
        return true;
    }
    std::str::from_utf8(slice).is_err()
}

pub fn read_text_best_effort(path: &Path, normalize_eol: bool) -> Result<String> {
    let bytes = fs::read(path)?;
    let content = match String::from_utf8(bytes.clone()) {
        Ok(s) => s,
        Err(_) => {
            let (res, _, _) = WINDOWS_1252.decode(&bytes);
            res.into_owned()
        }
    };

    if normalize_eol {
        Ok(content.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Ok(content)
    }
}

pub fn file_bytes_equal(p1: &Path, p2: &Path) -> bool {
    let hash_file = |p: &Path| -> Option<String> {
        let mut file = File::open(p).ok()?;
        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher).ok()?;
        Some(hex::encode(hasher.finalize()))
    };

    match (hash_file(p1), hash_file(p2)) {
        (Some(h1), Some(h2)) => h1 == h2,
        _ => false,
    }
}

pub fn avoid_collision(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or(Path::new("."));

    let mut n = 1;
    loop {
        let candidate = parent.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

pub fn rel_parts_with_deleted_suffix(rel: &Path) -> PathBuf {
    let mut new_path = PathBuf::new();
    for comp in rel.components() {
        if let std::path::Component::Normal(name) = comp {
            let s = name.to_string_lossy();
            new_path.push(format!("{s}.deleted"));
        } else {
            new_path.push(comp);
        }
    }
    new_path
}
