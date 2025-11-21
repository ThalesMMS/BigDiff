use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use similar::{ChangeTag, TextDiff};
use walkdir::WalkDir;

use crate::cli::Options;
use crate::comment::{comment_style_for, CommentStyle};
use crate::scanner::{scan_dir, ScanResult};
use crate::utils::{
    avoid_collision, file_bytes_equal, is_probably_binary, read_text_best_effort,
    rel_parts_with_deleted_suffix,
};

#[derive(Default, Debug)]
pub struct Counters {
    pub same: usize,
    pub new_files: usize,
    pub del_files: usize,
    pub mod_text: usize,
    pub mod_binary: usize,
    pub del_dirs: usize,
}

pub fn annotate_text_diff(
    a_path: &Path,
    b_path: &Path,
    style: &CommentStyle,
    normalize_eol: bool,
) -> Result<String> {
    let a_text = read_text_best_effort(a_path, normalize_eol)?;
    let b_text = read_text_best_effort(b_path, normalize_eol)?;

    let diff = TextDiff::from_lines(&a_text, &b_text);
    let mut output = String::new();

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => output.push_str(change.value()),
            ChangeTag::Delete => output.push_str(&style.deleted_line(change.value())),
            ChangeTag::Insert => output.push_str(&style.append_new_suffix(change.value())),
        }
    }
    Ok(output)
}

fn copy_deleted_tree(
    head_rel: &Path,
    scan_a: &ScanResult,
    out_root: &Path,
    counters: &mut Counters,
) -> HashSet<PathBuf> {
    let mut processed = HashSet::new();
    let head_abs = scan_a.root.join(head_rel);

    for entry in WalkDir::new(&head_abs).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel_from_root = match path.strip_prefix(&scan_a.root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if !rel_from_root.starts_with(head_rel) {
            continue;
        }

        let dest_path = out_root.join(rel_parts_with_deleted_suffix(rel_from_root));

        if entry.file_type().is_dir() {
            let _ = fs::create_dir_all(&dest_path);
            if rel_from_root == head_rel {
                counters.del_dirs += 1;
            }
        } else {
            let mut dest_file = dest_path;
            if let Some(name) = dest_file.file_name() {
                let mut new_name = name.to_os_string();
                new_name.push(".deleted");
                dest_file.set_file_name(new_name);
            }

            if let Some(parent) = dest_file.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let dest_file = avoid_collision(&dest_file);
            let _ = fs::copy(path, &dest_file);
            counters.del_files += 1;
            processed.insert(rel_from_root.to_path_buf());
        }
    }
    processed
}

pub fn run_bigdiff(
    a_root: &Path,
    b_root: &Path,
    out_root: &Path,
    opts: &Options,
) -> Result<Counters> {
    let scan_a = scan_dir(a_root, &opts.ignore_patterns);
    let scan_b = scan_dir(b_root, &opts.ignore_patterns);

    let mut counters = Counters::default();

    let del_dirs_all: Vec<_> = scan_a
        .dirs
        .iter()
        .filter(|d| !scan_b.dirs.contains(*d))
        .collect();

    let mut head_del_dirs: Vec<&PathBuf> = Vec::new();
    let mut sorted_dirs = del_dirs_all.clone();
    sorted_dirs.sort_by_key(|p| p.components().count());

    for d in sorted_dirs {
        if !head_del_dirs
            .iter()
            .any(|head| d.starts_with(head) && d != *head)
        {
            head_del_dirs.push(d);
        }
    }

    let mut processed_deleted_files = HashSet::new();
    for head in head_del_dirs {
        let processed = copy_deleted_tree(head, &scan_a, out_root, &mut counters);
        processed_deleted_files.extend(processed);
    }

    for (rel_a, abs_a) in &scan_a.files {
        if processed_deleted_files.contains(rel_a) {
            continue;
        }
        if !scan_b.files.contains_key(rel_a) {
            let mut dst = out_root.join(rel_a);
            if let Some(name) = dst.file_name() {
                let mut new_name = name.to_os_string();
                new_name.push(".deleted");
                dst.set_file_name(new_name);
            }
            if let Some(p) = dst.parent() {
                fs::create_dir_all(p)?;
            }
            dst = avoid_collision(&dst);
            fs::copy(abs_a, dst)?;
            counters.del_files += 1;
        }
    }

    for (rel_b, abs_b) in &scan_b.files {
        if !scan_a.files.contains_key(rel_b) {
            let mut dst = out_root.join(rel_b);
            if let Some(name) = dst.file_name() {
                let mut new_name = name.to_os_string();
                new_name.push(".new");
                dst.set_file_name(new_name);
            }
            if let Some(p) = dst.parent() {
                fs::create_dir_all(p)?;
            }
            dst = avoid_collision(&dst);
            fs::copy(abs_b, dst)?;
            counters.new_files += 1;
        }
    }

    let common_files: Vec<_> = scan_a
        .files
        .keys()
        .filter(|k| scan_b.files.contains_key(*k))
        .collect();

    for rel in common_files {
        let a_file = &scan_a.files[rel];
        let b_file = &scan_b.files[rel];

        if file_bytes_equal(a_file, b_file) {
            counters.same += 1;
            continue;
        }

        let style = comment_style_for(rel);
        let mut dst = out_root.join(rel);
        if let Some(name) = dst.file_name() {
            let mut new_name = name.to_os_string();
            new_name.push(".modified");
            dst.set_file_name(new_name);
        }
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p)?;
        }
        dst = avoid_collision(&dst);

        let size_b = fs::metadata(b_file)?.len();
        let is_bin = is_probably_binary(b_file);

        if is_bin || size_b > opts.max_text_size {
            fs::copy(b_file, &dst)?;
            counters.mod_binary += 1;

            let mut note_path = dst.clone();
            if let Some(name) = note_path.file_name() {
                let mut new_name = name.to_os_string();
                new_name.push(".NOTE.txt");
                note_path.set_file_name(new_name);
            }
            let note_content = format!(
                "File treated as binary or too large for line diff.\n\
Base origin (A): {:?}\n\
Target origin (B): {:?}\n\
Size: {} bytes\n\
Strategy: direct copy from target to '.modified'.\n",
                a_file, b_file, size_b
            );
            fs::write(note_path, note_content)?;
        } else {
            let annotated = annotate_text_diff(a_file, b_file, &style, opts.normalize_eol)?;
            fs::write(dst, annotated)?;
            counters.mod_text += 1;
        }
    }

    Ok(counters)
}
