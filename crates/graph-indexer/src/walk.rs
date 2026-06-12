use crate::model::WalkEntry;
use crate::IndexError;
use ignore::WalkBuilder;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SKIP_DIRS: &[&str] = &[".graph"];

/// Walk all files and directories under `root`, honoring `.gitignore` when present.
pub fn walk_repo(root: &Path) -> Result<Vec<WalkEntry>, IndexError> {
    let root = root
        .canonicalize()
        .map_err(|e| IndexError::PathNotFound(format!("{}: {e}", root.display())))?;

    let mut entries: BTreeMap<PathBuf, WalkEntry> = BTreeMap::new();
    entries.insert(
        PathBuf::from("."),
        WalkEntry {
            relative_path: PathBuf::from("."),
            is_dir: true,
            size_bytes: None,
        },
    );

    let walker = WalkBuilder::new(&root)
        .hidden(true)
        .git_ignore(true)
        .require_git(false)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if entry.depth() > 0 && SKIP_DIRS.contains(&name.as_ref()) {
                return false;
            }
            true
        })
        .build();

    for result in walker {
        let entry = result?;
        let path = entry.path();

        if path == root {
            continue;
        }

        let relative = path
            .strip_prefix(&root)
            .map_err(|e| IndexError::Io(std::io::Error::other(e.to_string())))?
            .to_path_buf();

        if should_skip_relative(&relative) {
            continue;
        }

        ensure_parent_dirs(&mut entries, &relative);

        let metadata = entry.metadata().ok();
        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size_bytes = if is_dir {
            None
        } else {
            metadata.map(|m| m.len())
        };

        entries.insert(
            relative.clone(),
            WalkEntry {
                relative_path: relative,
                is_dir,
                size_bytes,
            },
        );
    }

    Ok(entries.into_values().collect())
}

fn should_skip_relative(relative: &Path) -> bool {
    relative
        .components()
        .any(|c| c.as_os_str() == ".graph" || c.as_os_str() == "target")
}

fn ensure_parent_dirs(entries: &mut BTreeMap<PathBuf, WalkEntry>, relative: &Path) {
    let mut ancestor = PathBuf::new();
    for component in relative.parent().unwrap_or(Path::new("")).components() {
        ancestor.push(component);
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        entries.entry(ancestor.clone()).or_insert(WalkEntry {
            relative_path: ancestor.clone(),
            is_dir: true,
            size_bytes: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn respects_gitignore() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("visible.txt"), "ok").unwrap();
        fs::create_dir(root.join("ignored")).unwrap();
        fs::write(root.join("ignored/secret.txt"), "no").unwrap();
        fs::write(root.join(".gitignore"), "ignored/\n").unwrap();

        let entries = walk_repo(root).unwrap();
        let files: Vec<_> = entries
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.relative_path.to_string_lossy().into_owned())
            .collect();

        assert!(files.contains(&"visible.txt".to_string()));
        assert!(!files.iter().any(|p| p.contains("ignored")));
    }
}
