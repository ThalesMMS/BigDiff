use anyhow::{Context, Result};
use clap::Parser;
use glob::Pattern;
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// --------------------------- 
// Structures and CLI
// --------------------------- 

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Base directory (A)
    base_dir: PathBuf,

    /// Target directory (B)
    target_dir: PathBuf,

    /// Output directory (Differences)
    output_dir: PathBuf,

    /// Glob patterns to ignore (can be repeated or comma separated)
    #[arg(short, long, value_delimiter = ',', num_args = 1..)]
    ignore: Vec<String>,

    /// Normalize EOL (CRLF/LF) before text comparison
    #[arg(short = 'E', long)]
    normalize_eol: bool,

    /// Max size (in bytes) for text diff per file (e.g., 5MB, 102400)
    #[arg(short = 'S', long, default_value = "5MB")]
    max_text_size: String,

    /// Do not write anything; only print a summary of what would be done
    #[arg(long)]
    dry_run: bool,
}

#[derive(Default, Debug)]
struct Counters {
    same: usize,
    new_files: usize,
    del_files: usize,
    mod_text: usize,
    mod_binary: usize,
    del_dirs: usize,
}

struct Options {
    normalize_eol: bool,
    max_text_size: u64,
    ignore_patterns: Vec<Pattern>,
    dry_run: bool,
}

// --------------------------- 
// Comment Style Logic
// --------------------------- 

enum CommentStyle {
    LinePrefix { prefix: String, new_suffix: String },
    Block { open: String, close: String, new_block: String },
}

impl CommentStyle {
    fn deleted_line(&self, line: &str) -> String {
        let (content, end) = split_newline(line);
        match self {
            CommentStyle::LinePrefix { prefix, .. } => format!("{}DELETED: {}{}", prefix, content, end),
            CommentStyle::Block { open, close, .. } => format!("{} DELETED: {} {}{}", open, content, close, end),
        }
    }

    fn append_new_suffix(&self, line: &str) -> String {
        let (content, end) = split_newline(line);
        match self {
            CommentStyle::LinePrefix { new_suffix, .. } => format!("{}{}{}", content, new_suffix, end),
            CommentStyle::Block { new_block, .. } => format!("{} {}{}", content, new_block, end),
        }
    }
}

fn split_newline(s: &str) -> (&str, &str) {
    if let Some(stripped) = s.strip_suffix('\n') {
        (stripped, "\n")
    } else {
        (s, "")
    }
}

fn comment_style_for(path: &Path) -> CommentStyle {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let ext = format!(".{}", ext);

    let slash_exts = [".c", ".h", ".cpp", ".hpp", ".cc", ".java", ".js", ".ts", ".tsx", ".cs", ".swift", ".go", ".kt", ".kts", ".scala", ".dart", ".php", ".rs"];
    let hash_exts = [".py", ".sh", ".rb", ".r", ".ps1", ".toml", ".yaml", ".yml", ".cfg", ".gitignore", ".dockerignore"];
    let dash_exts = [".sql", ".hs", ".lua"];
    let percent_exts = [".tex", ".m"];
    let semicolon_exts = [".ini"];
    let csv_like = [".csv", ".tsv"];
    let text_like = [".txt", ".log", ".conf", ".md"]; // MD treated as line here for simplicity unless strictly block needed

    // Block styles
    let html_exts = [".html", ".htm", ".xml", ".xhtml", ".svg"];
    let cblock_exts = [".css", ".scss", ".less"];
    let json_exts = [".json"];

    if slash_exts.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix { prefix: "// ".into(), new_suffix: " // NEW".into() };
    }
    if hash_exts.contains(&ext.as_str()) || text_like.contains(&ext.as_str()) || csv_like.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix { prefix: "# ".into(), new_suffix: " # NEW".into() };
    }
    if dash_exts.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix { prefix: "-- ".into(), new_suffix: " -- NEW".into() };
    }
    if percent_exts.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix { prefix: "% ".into(), new_suffix: " % NEW".into() };
    }
    if semicolon_exts.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix { prefix: "; ".into(), new_suffix: " ; NEW".into() };
    }
    
    if html_exts.contains(&ext.as_str()) {
        return CommentStyle::Block { open: "".into(), new_block: "".into() };
    }
    if cblock_exts.contains(&ext.as_str()) || json_exts.contains(&ext.as_str()) {
        return CommentStyle::Block { open: "/*".into(), close: "*/".into(), new_block: "/* NEW */".into() };
    }

    // Fallback
    CommentStyle::LinePrefix { prefix: "# ".into(), new_suffix: " # NEW".into() }
}

// --------------------------- 
// Utils
// --------------------------- 

fn parse_size(s: &str) -> u64 {
    let s = s.trim().to_lowercase();
    let units = [
        ("gib", 1024u64.pow(3)), ("g", 1000u64.pow(3)),
        ("mib", 1024u64.pow(2)), ("m", 1000u64.pow(2)),
        ("kib", 1024), ("k", 1000), ("kb", 1000), ("mb", 1000u64.pow(2)), ("gb", 1000u64.pow(3)), ("b", 1)
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

fn is_probably_binary(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return true,
    };
    let mut buffer = [0u8; 4096];
    let n = match file.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return true,
    };
    if n == 0 { return false; } // Empty file is text
    
    let slice = &buffer[..n];
    if slice.contains(&0) {
        return true;
    }
    // Try validating UTF-8
    std::str::from_utf8(slice).is_err()
}

fn read_text_best_effort(path: &Path, normalize_eol: bool) -> Result<String> {
    let bytes = fs::read(path)?;
    // Try UTF-8
    let content = match String::from_utf8(bytes.clone()) {
        Ok(s) => s,
        Err(_) => {
            // Fallback: decode as LATIN1 (Windows-1252) using encoding_rs
            let (res, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
            res.into_owned()
        }
    };

    if normalize_eol {
        Ok(content.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Ok(content)
    }
}

fn file_bytes_equal(p1: &Path, p2: &Path) -> bool {
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

fn avoid_collision(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|s| s.to_str()).map(|e| format!(".{}", e)).unwrap_or_default();
    let parent = path.parent().unwrap_or(Path::new("."));
    
    let mut n = 1;
    loop {
        let candidate = parent.join(format!("{} ({}){}", stem, n, ext));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

fn rel_parts_with_deleted_suffix(rel: &Path) -> PathBuf {
    let mut new_path = PathBuf::new();
    for comp in rel.components() {
        if let std::path::Component::Normal(name) = comp {
            let s = name.to_string_lossy();
            new_path.push(format!("{}.deleted", s));
        } else {
            new_path.push(comp);
        }
    }
    new_path
}

// --------------------------- 
// Scanner
// --------------------------- 

struct ScanResult {
    files: HashMap<PathBuf, PathBuf>, // rel -> abs
    dirs: HashSet<PathBuf>,           // rel
    root: PathBuf,
}

fn is_ignored(rel: &Path, patterns: &[Pattern]) -> bool {
    let name = rel.file_name().and_then(|s| s.to_str()).unwrap_or("");
    // Default ignores
    if [".git", "__pycache__", ".DS_Store", "Thumbs.db"].contains(&name) {
        return true;
    }
    let s_rel = rel.to_string_lossy().replace('\\', "/"); // Glob uses forward slash
    for pat in patterns {
        if pat.matches(&s_rel) || pat.matches(name) {
            return true;
        }
    }
    false
}

fn scan_dir(root: &Path, patterns: &[Pattern]) -> ScanResult {
    let mut files = HashMap::new();
    let mut dirs = HashSet::new();

    // WalkDir is great but for filtering directories before entering them (like os.walk modification),
    // we use filter_entry.
    let walker = WalkDir::new(root).follow_links(false).into_iter();
    
    for entry in walker.filter_entry(|e| {
        let path = e.path();
        if let Ok(rel) = path.strip_prefix(root) {
            if rel == Path::new("") { return true; }
            !is_ignored(rel, patterns)
        } else {
            true
        }
    }) {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Ok(rel) = path.strip_prefix(root) {
                if rel == Path::new("") { continue; }
                
                if path.is_dir() {
                    dirs.insert(rel.to_path_buf());
                } else if path.is_file() {
                    files.insert(rel.to_path_buf(), path.to_path_buf());
                }
            }
        }
    }

    ScanResult { files, dirs, root: root.to_path_buf() }
}

// --------------------------- 
// Core
// --------------------------- 

fn annotate_text_diff(a_path: &Path, b_path: &Path, style: &CommentStyle, normalize_eol: bool) -> Result<String> {
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

fn copy_deleted_tree(head_rel: &Path, scan_a: &ScanResult, out_root: &Path, counters: &mut Counters) -> HashSet<PathBuf> {
    let mut processed = HashSet::new();
    let head_abs = scan_a.root.join(head_rel);
    
    for entry in WalkDir::new(&head_abs).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel_from_root = match path.strip_prefix(&scan_a.root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        
        // Check if it is inside the subtree (always true here, but good to guarantee)
        if !rel_from_root.starts_with(head_rel) { continue; }
        
        // Output path with .deleted in all new parts
        let dest_path = out_root.join(rel_parts_with_deleted_suffix(rel_from_root));
        
        if entry.file_type().is_dir() {
            if let Err(_) = fs::create_dir_all(&dest_path) { continue; }
            if rel_from_root == head_rel {
                counters.del_dirs += 1;
            }
        } else {
            // File: add .deleted to name
            let mut dest_file = dest_path;
            if let Some(name) = dest_file.file_name() {
                let mut new_name = name.to_os_string();
                new_name.push(".deleted");
                dest_file.set_file_name(new_name);
            }
            
            // Prepare parent directory
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

fn run_bigdiff(a_root: &Path, b_root: &Path, out_root: &Path, opts: &Options) -> Result<Counters> {
    let scan_a = scan_dir(a_root, &opts.ignore_patterns);
    let scan_b = scan_dir(b_root, &opts.ignore_patterns);
    
    let mut counters = Counters::default();

    // 1. Deleted directories
    let del_dirs_all: Vec<_> = scan_a.dirs.iter()
        .filter(|d| !scan_b.dirs.contains(*d))
        .collect();
    
    // Filter only heads (top-level deleted)
    let mut head_del_dirs: Vec<&PathBuf> = Vec::new();
    let mut sorted_dirs = del_dirs_all.clone();
    sorted_dirs.sort_by_key(|p| p.components().count()); // shallowest first

    for d in sorted_dirs {
        if !head_del_dirs.iter().any(|head| d.starts_with(head) && d != *head) {
            head_del_dirs.push(d);
        }
    }

    let mut processed_deleted_files = HashSet::new();
    for head in head_del_dirs {
        let processed = copy_deleted_tree(head, &scan_a, out_root, &mut counters);
        processed_deleted_files.extend(processed);
    }

    // 2. Deleted files (loose)
    for (rel_a, abs_a) in &scan_a.files {
        if processed_deleted_files.contains(rel_a) { continue; }
        if !scan_b.files.contains_key(rel_a) {
            let mut dst = out_root.join(rel_a);
            if let Some(name) = dst.file_name() {
                let mut new_name = name.to_os_string();
                new_name.push(".deleted");
                dst.set_file_name(new_name);
            }
            if let Some(p) = dst.parent() { fs::create_dir_all(p)?; }
            dst = avoid_collision(&dst);
            fs::copy(abs_a, dst)?;
            counters.del_files += 1;
        }
    }

    // 3. New files
    for (rel_b, abs_b) in &scan_b.files {
        if !scan_a.files.contains_key(rel_b) {
            let mut dst = out_root.join(rel_b);
            if let Some(name) = dst.file_name() {
                let mut new_name = name.to_os_string();
                new_name.push(".new");
                dst.set_file_name(new_name);
            }
            if let Some(p) = dst.parent() { fs::create_dir_all(p)?; }
            dst = avoid_collision(&dst);
            fs::copy(abs_b, dst)?;
            counters.new_files += 1;
        }
    }

    // 4. Common files (modified or equal)
    let common_files: Vec<_> = scan_a.files.keys()
        .filter(|k| scan_b.files.contains_key(*k))
        .collect();
    
    for rel in common_files {
        let a_file = &scan_a.files[rel];
        let b_file = &scan_b.files[rel];

        if file_bytes_equal(a_file, b_file) {
            counters.same += 1;
            continue;
        }

        // Modified
        let style = comment_style_for(rel);
        let mut dst = out_root.join(rel);
        if let Some(name) = dst.file_name() {
            let mut new_name = name.to_os_string();
            new_name.push(".modified");
            dst.set_file_name(new_name);
        }
        if let Some(p) = dst.parent() { fs::create_dir_all(p)?; }
        dst = avoid_collision(&dst);

        let size_b = fs::metadata(b_file)?.len();
        let is_bin = is_probably_binary(b_file);

        if is_bin || size_b > opts.max_text_size {
            fs::copy(b_file, &dst)?;
            counters.mod_binary += 1;
            
            // Note
            let mut note_path = dst.clone();
            if let Some(name) = note_path.file_name() {
                let mut new_name = name.to_os_string();
                new_name.push(".NOTE.txt");
                note_path.set_file_name(new_name);
            }
            let note_content = format!(
                "File treated as binary or too large for line diff.\n\                                Base origin (A): {{:?}}\n\                                Target origin (B): {{:?}}\n\                                Size: {} bytes\n\                                Strategy: direct copy from target to '.modified'.\n",
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

// --------------------------- 
// Main
// --------------------------- 

fn main() -> Result<()> {
    let args = Args::parse();

    let a_root = args.base_dir.canonicalize().context("Invalid base_dir")?;
    let b_root = args.target_dir.canonicalize().context("Invalid target_dir")?;
    let out_root = args.output_dir; // Do not canonicalize if it doesn't exist, resolve later

    if a_root == b_root {
        anyhow::bail!("base_dir and target_dir cannot be the same directory.");
    }
    if out_root.exists() {
        let out_abs = out_root.canonicalize()?;
        if out_abs == a_root || out_abs == b_root || out_abs.starts_with(&a_root) || out_abs.starts_with(&b_root) {
            anyhow::bail!("output_dir cannot be inside base_dir/target_dir nor be equal to them.");
        }
    } else {
        fs::create_dir_all(&out_root)?;
    }

    let patterns: Vec<Pattern> = args.ignore.iter()
        .map(|s| Pattern::new(s).expect("Invalid glob pattern"))
        .collect();

    let opts = Options {
        normalize_eol: args.normalize_eol,
        max_text_size: parse_size(&args.max_text_size),
        ignore_patterns: patterns,
        dry_run: args.dry_run,
    };

    if opts.dry_run {
        println!("== DRY RUN (Rust Simulation) ==");
        // Simplified: only calls scan and shows basic numbers
        let scan_a = scan_dir(&a_root, &opts.ignore_patterns);
        let scan_b = scan_dir(&b_root, &opts.ignore_patterns);
        
        let only_a = scan_a.files.keys().filter(|k| !scan_b.files.contains_key(*k)).count();
        let only_b = scan_b.files.keys().filter(|k| !scan_a.files.contains_key(*k)).count();
        let common = scan_a.files.keys().filter(|k| scan_b.files.contains_key(*k)).count();
        
        println!("Files only in Base (would be deleted): {{}}", only_a);
        println!("Files only in Target (would be new): {{}}", only_b);
        println!("Common files (would be checked): {{}}", common);
        return Ok(());
    }

    let counters = run_bigdiff(&a_root, &b_root, &out_root, &opts)?;

    println!("== BigDiff (Rust): Summary ==");
    println!("Equal (omitted):      {{}}", counters.same);
    println!("New (.new):           {{}}", counters.new_files);
    println!("Deleted (.deleted):   {{}}", counters.del_files);
    println!("Modified text:        {{}}", counters.mod_text);
    println!("Modified binary:      {{}}", counters.mod_binary);
    println!("Deleted dirs:         {{}}", counters.del_dirs);
    println!("Output at:            {{:?}}", out_root);

    Ok(())
}
