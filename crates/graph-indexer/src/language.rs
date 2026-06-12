use std::path::Path;

/// Detect language id from file path. Returns `"unknown"` for unrecognized extensions.
pub fn detect_language(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);

    match ext.as_deref() {
        None => "unknown",
        Some("py") => "python",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => "javascript",
        Some("go") => "go",
        Some("rs") => "rust",
        Some("rb") => "ruby",
        Some("java") => "java",
        Some("md") | Some("markdown") => "markdown",
        Some("txt") => "text",
        Some("json") => "json",
        Some("yaml") | Some("yml") => "yaml",
        Some("toml") => "toml",
        Some("xml") => "xml",
        Some("html") | Some("htm") => "html",
        Some("css") => "css",
        Some("sql") => "sql",
        Some("sh") | Some("bash") | Some("zsh") => "shell",
        Some("dart") => "dart",
        Some("swift") => "swift",
        Some("kt") | Some("kts") => "kotlin",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") => "cpp",
        Some("cs") => "csharp",
        Some("php") => "php",
        Some("lua") => "lua",
        Some("r") => "r",
        Some("scala") => "scala",
        Some("zig") => "zig",
        Some("ex") | Some("exs") => "elixir",
        Some("erl") => "erlang",
        Some("hs") => "haskell",
        Some("ml") | Some("mli") => "ocaml",
        Some("vue") => "vue",
        Some("svelte") => "svelte",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_extensions_map_correctly() {
        assert_eq!(detect_language(Path::new("a.xyz")), "unknown");
        assert_eq!(detect_language(Path::new("a.dat")), "unknown");
        assert_eq!(detect_language(Path::new("a.txt")), "text");
        assert_eq!(detect_language(Path::new("main.py")), "python");
    }
}
