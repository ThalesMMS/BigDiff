use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use glob::Pattern;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct ScanResult {
    pub files: HashMap<PathBuf, PathBuf>, // rel -> abs
    pub dirs: HashSet<PathBuf>,           // rel
    pub root: PathBuf,
}

fn is_ignored(rel: &Path, patterns: &[Pattern]) -> bool {
    let name = rel.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if [".git", "__pycache__", ".DS_Store", "Thumbs.db"].contains(&name) {
        return true;
    }
    let s_rel = rel.to_string_lossy().replace('\\', "/");
    for pat in patterns {
        if pat.matches(&s_rel) || pat.matches(name) {
            return true;
        }
    }
    false
}

pub fn scan_dir(root: &Path, patterns: &[Pattern]) -> ScanResult {
    let mut files = HashMap::new();
    let mut dirs = HashSet::new();

    let walker = WalkDir::new(root).follow_links(false).into_iter();

    for entry in walker.filter_entry(|e| {
        let path = e.path();
        if let Ok(rel) = path.strip_prefix(root) {
            if rel == Path::new("") {
                return true;
            }
            !is_ignored(rel, patterns)
        } else {
            true
        }
    }) {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Ok(rel) = path.strip_prefix(root) {
                if rel == Path::new("") {
                    continue;
                }

                if path.is_dir() {
                    dirs.insert(rel.to_path_buf());
                } else if path.is_file() {
                    files.insert(rel.to_path_buf(), path.to_path_buf());
                }
            }
        }
    }

    ScanResult {
        files,
        dirs,
        root: root.to_path_buf(),
    }
}
