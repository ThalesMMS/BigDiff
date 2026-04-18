#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use bigdiff::{run_bigdiff, Options};

#[cfg(unix)]
fn unique_temp_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("bigdiff-output-safety-{stamp}"))
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_output_subdirectories() {
    let temp = unique_temp_dir();
    let base = temp.join("base");
    let target = temp.join("target");
    let out = temp.join("out");
    let escape = temp.join("escape");

    fs::create_dir_all(base.join("nested")).unwrap();
    fs::create_dir_all(target.join("nested")).unwrap();
    fs::create_dir_all(&out).unwrap();
    fs::create_dir_all(&escape).unwrap();

    fs::write(base.join("nested/demo.txt"), "old\n").unwrap();
    fs::write(target.join("nested/demo.txt"), "new\n").unwrap();

    std::os::unix::fs::symlink(&escape, out.join("nested")).unwrap();

    let opts = Options {
        normalize_eol: false,
        max_text_size: 1_000_000,
        ignore_patterns: vec![],
        dry_run: false,
    };

    let err = run_bigdiff(&base, &target, &out, &opts)
        .unwrap_err()
        .to_string();
    assert!(err.contains("symlinked output path component"));
    assert!(!escape.join("demo.txt.modified").exists());

    fs::remove_dir_all(temp).unwrap();
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_output_ancestors() {
    let temp = unique_temp_dir();
    let base = temp.join("base");
    let target = temp.join("target");
    let real_parent = temp.join("real-parent");
    let link_parent = temp.join("link-parent");
    let out = link_parent.join("out");

    fs::create_dir_all(base.join("nested")).unwrap();
    fs::create_dir_all(target.join("nested")).unwrap();
    fs::create_dir_all(&real_parent).unwrap();

    fs::write(base.join("nested/demo.txt"), "old\n").unwrap();
    fs::write(target.join("nested/demo.txt"), "new\n").unwrap();

    std::os::unix::fs::symlink(&real_parent, &link_parent).unwrap();

    let opts = Options {
        normalize_eol: false,
        max_text_size: 1_000_000,
        ignore_patterns: vec![],
        dry_run: false,
    };

    let err = run_bigdiff(&base, &target, &out, &opts)
        .unwrap_err()
        .to_string();
    assert!(err.contains("symlinked output path component"));
    assert!(!real_parent.join("out/nested/demo.txt.modified").exists());

    fs::remove_dir_all(temp).unwrap();
}
