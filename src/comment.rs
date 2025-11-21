use std::path::Path;

#[derive(Debug)]
pub enum CommentStyle {
    LinePrefix {
        prefix: String,
        new_suffix: String,
    },
    Block {
        open: String,
        close: String,
        new_block: String,
    },
}

impl CommentStyle {
    pub fn deleted_line(&self, line: &str) -> String {
        let (content, end) = split_newline(line);
        match self {
            CommentStyle::LinePrefix { prefix, .. } => {
                format!("{prefix}DELETED: {content}{end}")
            }
            CommentStyle::Block { open, close, .. } => {
                format!("{open} DELETED: {content} {close}{end}")
            }
        }
    }

    pub fn append_new_suffix(&self, line: &str) -> String {
        let (content, end) = split_newline(line);
        match self {
            CommentStyle::LinePrefix { new_suffix, .. } => {
                format!("{content}{new_suffix}{end}")
            }
            CommentStyle::Block { new_block, .. } => format!("{content} {new_block}{end}"),
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

pub fn comment_style_for(path: &Path) -> CommentStyle {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let ext = format!(".{ext}");

    let slash_exts = [
        ".c", ".h", ".cpp", ".hpp", ".cc", ".java", ".js", ".ts", ".tsx", ".cs", ".swift", ".go",
        ".kt", ".kts", ".scala", ".dart", ".php", ".rs",
    ];
    let hash_exts = [
        ".py",
        ".sh",
        ".rb",
        ".r",
        ".ps1",
        ".toml",
        ".yaml",
        ".yml",
        ".cfg",
        ".gitignore",
        ".dockerignore",
    ];
    let dash_exts = [".sql", ".hs", ".lua"];
    let percent_exts = [".tex", ".m"];
    let semicolon_exts = [".ini"];
    let csv_like = [".csv", ".tsv"];
    let text_like = [".txt", ".log", ".conf", ".md"];

    let html_exts = [".html", ".htm", ".xml", ".xhtml", ".svg"];
    let cblock_exts = [".css", ".scss", ".less"];
    let json_exts = [".json"];

    if slash_exts.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix {
            prefix: "// ".into(),
            new_suffix: " // NEW".into(),
        };
    }
    if hash_exts.contains(&ext.as_str())
        || text_like.contains(&ext.as_str())
        || csv_like.contains(&ext.as_str())
    {
        return CommentStyle::LinePrefix {
            prefix: "# ".into(),
            new_suffix: " # NEW".into(),
        };
    }
    if dash_exts.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix {
            prefix: "-- ".into(),
            new_suffix: " -- NEW".into(),
        };
    }
    if percent_exts.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix {
            prefix: "% ".into(),
            new_suffix: " % NEW".into(),
        };
    }
    if semicolon_exts.contains(&ext.as_str()) {
        return CommentStyle::LinePrefix {
            prefix: "; ".into(),
            new_suffix: " ; NEW".into(),
        };
    }

    if html_exts.contains(&ext.as_str()) {
        return CommentStyle::Block {
            open: "<!--".into(),
            close: "-->".into(),
            new_block: "<!-- NEW -->".into(),
        };
    }
    if cblock_exts.contains(&ext.as_str()) || json_exts.contains(&ext.as_str()) {
        return CommentStyle::Block {
            open: "/*".into(),
            close: "*/".into(),
            new_block: "/* NEW */".into(),
        };
    }

    CommentStyle::LinePrefix {
        prefix: "# ".into(),
        new_suffix: " # NEW".into(),
    }
}
