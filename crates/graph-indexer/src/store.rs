use crate::model::{
    Capabilities, EdgeKind, ExtractedSymbol, GraphEdge, GraphNode, LanguageCapability, NodeKind,
    ResolvedEdge, Tier, WalkEntry, edge_id, node_id_for_path, symbol_id,
};
use crate::IndexError;
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet};

pub struct GraphStore {
    conn: Connection,
}

impl GraphStore {
    pub fn open_in_memory() -> Result<Self, IndexError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open(path: &std::path::Path) -> Result<Self, IndexError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), IndexError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                parent_file TEXT,
                line INTEGER,
                extension TEXT,
                language_id TEXT NOT NULL,
                size_bytes INTEGER
            );
            CREATE TABLE IF NOT EXISTS edges (
                id TEXT PRIMARY KEY,
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                confidence REAL
            );
            CREATE INDEX IF NOT EXISTS idx_nodes_language ON nodes(language_id);
            CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
            CREATE INDEX IF NOT EXISTS idx_nodes_parent_file ON nodes(parent_file);
            CREATE INDEX IF NOT EXISTS idx_nodes_name_file ON nodes(parent_file, name, kind);
            CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_id);
            CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
            ",
        )?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), IndexError> {
        self.conn.execute("DELETE FROM edges", [])?;
        self.conn.execute("DELETE FROM nodes", [])?;
        Ok(())
    }

    pub fn node_exists(&self, id: &str) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM nodes WHERE id = ?1",
                params![id],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn find_function_symbol_id(&self, file: &str, name: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT id FROM nodes WHERE parent_file = ?1 AND name = ?2 AND kind = 'function' ORDER BY line LIMIT 1",
                params![file, name],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn find_route_symbol_id(&self, route_name: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT id FROM nodes WHERE kind = 'route' AND name = ?1 ORDER BY line LIMIT 1",
                params![route_name],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn find_enclosing_function(&self, file: &str, line: u32) -> Option<String> {
        self.conn
            .query_row(
                "SELECT id FROM nodes
                 WHERE parent_file = ?1 AND kind = 'function' AND line <= ?2
                 ORDER BY line DESC LIMIT 1",
                params![file, line],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn find_import_symbol_id(&self, file: &str, local_name: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT id FROM nodes WHERE parent_file = ?1 AND kind = 'import' AND (name LIKE '%' || ?2 || '%') ORDER BY line LIMIT 1",
                params![file, local_name],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn set_l2_languages(&self, _languages: std::collections::HashSet<String>) -> Result<(), IndexError> {
        // Tier is derived from semantic edges in capabilities().
        Ok(())
    }

    pub fn insert_walk_entries(
        &self,
        entries: &[WalkEntry],
        language_for: impl Fn(&WalkEntry) -> &str,
    ) -> Result<(usize, usize), IndexError> {
        self.clear()?;

        let mut file_count = 0usize;
        let mut unknown_extension_count = 0usize;

        for entry in entries {
            let relative = entry.relative_path.to_string_lossy();
            let relative_str = if relative == "." {
                ".".to_string()
            } else {
                relative.into_owned()
            };

            let name = entry
                .relative_path
                .file_name()
                .map(std::ffi::OsStr::to_string_lossy)
                .map(|n| n.into_owned())
                .unwrap_or_else(|| ".".to_string());

            let kind = if entry.is_dir {
                NodeKind::Directory
            } else {
                NodeKind::File
            };

            let extension = if entry.is_dir {
                None
            } else {
                entry
                    .relative_path
                    .extension()
                    .map(std::ffi::OsStr::to_string_lossy)
                    .map(|e| e.into_owned())
            };

            let language_id = if entry.is_dir {
                "directory".to_string()
            } else {
                language_for(entry).to_string()
            };

            if kind == NodeKind::File {
                file_count += 1;
                if language_id == "unknown" {
                    unknown_extension_count += 1;
                }
            }

            let id = node_id_for_path(&relative_str);
            self.insert_node(
                &id,
                kind,
                &name,
                &relative_str,
                None,
                None,
                extension.as_deref(),
                &language_id,
                entry.size_bytes,
            )?;
        }

        for entry in entries {
            if entry.relative_path.as_os_str() == "." {
                continue;
            }
            let child_path = entry.relative_path.to_string_lossy().into_owned();
            let parent_path = match entry.relative_path.parent() {
                None => ".".to_string(),
                Some(p) if p.as_os_str().is_empty() => ".".to_string(),
                Some(p) => p.to_string_lossy().into_owned(),
            };

            self.insert_edge(
                &node_id_for_path(&parent_path),
                &node_id_for_path(&child_path),
                EdgeKind::Contains,
                None,
            )?;
        }

        Ok((file_count, unknown_extension_count))
    }

    pub fn insert_symbols(&self, symbols: &[ExtractedSymbol]) -> Result<(), IndexError> {
        for sym in symbols {
            let id = symbol_id(&sym.parent_file, sym.kind, &sym.name, sym.line);
            self.insert_node(
                &id,
                sym.kind,
                &sym.name,
                &sym.parent_file,
                Some(sym.parent_file.as_str()),
                Some(sym.line),
                None,
                &sym.language_id,
                None,
            )?;

            let file_id = node_id_for_path(&sym.parent_file);
            self.insert_edge(&file_id, &id, EdgeKind::Contains, None)?;
        }
        Ok(())
    }

    pub fn insert_relations(&self, relations: &[ResolvedEdge]) -> Result<(), IndexError> {
        for rel in relations {
            if !self.node_exists(&rel.from_id) || !self.node_exists(&rel.to_id) {
                continue;
            }
            self.insert_edge(
                &rel.from_id,
                &rel.to_id,
                rel.kind,
                Some(rel.confidence),
            )?;
        }
        Ok(())
    }

    fn insert_node(
        &self,
        id: &str,
        kind: NodeKind,
        name: &str,
        relative_path: &str,
        parent_file: Option<&str>,
        line: Option<u32>,
        extension: Option<&str>,
        language_id: &str,
        size_bytes: Option<u64>,
    ) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO nodes (id, kind, name, relative_path, parent_file, line, extension, language_id, size_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                kind.as_str(),
                name,
                relative_path,
                parent_file,
                line,
                extension,
                language_id,
                size_bytes.map(|s| s as i64),
            ],
        )?;
        Ok(())
    }

    fn insert_edge(
        &self,
        from_id: &str,
        to_id: &str,
        kind: EdgeKind,
        confidence: Option<f32>,
    ) -> Result<(), IndexError> {
        let eid = edge_id(from_id, to_id, kind);
        self.conn.execute(
            "INSERT OR REPLACE INTO edges (id, from_id, to_id, kind, confidence) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![eid, from_id, to_id, kind.as_str(), confidence],
        )?;
        Ok(())
    }

    pub fn file_count(&self) -> Result<usize, IndexError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM nodes WHERE kind = 'file'",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn symbol_count(&self) -> Result<usize, IndexError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM nodes WHERE kind IN ('function', 'class', 'import', 'ui_element', 'route', 'branch')",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn capabilities(&self) -> Result<Capabilities, IndexError> {
        let mut file_counts: BTreeMap<String, usize> = BTreeMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT language_id, COUNT(*) FROM nodes WHERE kind = 'file' GROUP BY language_id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (id, count) = row?;
                file_counts.insert(id, count as usize);
            }
        }

        let mut symbol_langs: BTreeSet<String> = BTreeSet::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT language_id FROM nodes WHERE kind IN ('function', 'class', 'import', 'ui_element', 'route', 'branch')",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                symbol_langs.insert(row?);
            }
        }

        let mut l2_langs: BTreeSet<String> = BTreeSet::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT n.language_id
                 FROM edges e
                 JOIN nodes n ON n.id = e.from_id OR n.id = e.to_id
                 WHERE e.kind IN ('CALLS', 'IMPORTS') AND n.language_id != 'directory'",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                l2_langs.insert(row?);
            }
        }

        let mut l3_langs: BTreeSet<String> = BTreeSet::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT n.language_id
                 FROM edges e
                 JOIN nodes n ON n.id = e.from_id OR n.id = e.to_id
                 WHERE e.kind IN ('TRIGGERS', 'HANDLES') AND n.language_id != 'directory'",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                l3_langs.insert(row?);
            }
        }

        let mut l4_langs: BTreeSet<String> = BTreeSet::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT n.language_id
                 FROM edges e
                 JOIN nodes n ON n.id = e.from_id OR n.id = e.to_id
                 WHERE e.kind = 'FETCHES' AND n.language_id != 'directory'",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                l4_langs.insert(row?);
            }
        }

        let languages = file_counts
            .into_iter()
            .map(|(id, files)| {
                let tier = if l4_langs.contains(&id) {
                    Tier::L4
                } else if l3_langs.contains(&id) {
                    Tier::L3
                } else if l2_langs.contains(&id) {
                    Tier::L2
                } else if symbol_langs.contains(&id) {
                    Tier::L1
                } else {
                    Tier::L0
                };
                LanguageCapability { id, files, tier }
            })
            .collect();

        Ok(Capabilities { languages })
    }

    pub fn list_nodes(&self) -> Result<Vec<GraphNode>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, relative_path, parent_file, line, extension, language_id, size_bytes
             FROM nodes ORDER BY kind, relative_path, line, name",
        )?;

        let rows = stmt.query_map([], |row| row_to_node(row))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(IndexError::from)
    }

    pub fn list_files(&self) -> Result<Vec<GraphNode>, IndexError> {
        Ok(self
            .list_nodes()?
            .into_iter()
            .filter(|n| n.kind == NodeKind::File)
            .collect())
    }

    pub fn get_node(&self, id: &str) -> Result<Option<GraphNode>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, relative_path, parent_file, line, extension, language_id, size_bytes
             FROM nodes WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_node(&row)?));
        }
        Ok(None)
    }

    pub fn search_symbols(&self, query: &str, limit: usize) -> Result<Vec<GraphNode>, IndexError> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, relative_path, parent_file, line, extension, language_id, size_bytes
             FROM nodes
             WHERE kind IN ('function', 'class', 'import')
               AND (name LIKE ?1 ESCAPE '\\' OR relative_path LIKE ?1 ESCAPE '\\' OR id LIKE ?1 ESCAPE '\\')
             ORDER BY
               CASE WHEN name = ?2 THEN 0 WHEN name LIKE ?3 ESCAPE '\\' THEN 1 ELSE 2 END,
               name
             LIMIT ?4",
        )?;
        let prefix = format!("{query}%");
        let rows = stmt.query_map(params![pattern, query, prefix, limit as i64], row_to_node)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(IndexError::from)
    }

    pub fn edges_from(&self, id: &str, kind: Option<EdgeKind>) -> Result<Vec<GraphEdge>, IndexError> {
        match kind {
            Some(k) => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, from_id, to_id, kind, confidence FROM edges WHERE from_id = ?1 AND kind = ?2",
                )?;
                let rows = stmt.query_map(params![id, k.as_str()], row_to_edge)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(IndexError::from)
            }
            None => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, from_id, to_id, kind, confidence FROM edges WHERE from_id = ?1",
                )?;
                let rows = stmt.query_map(params![id], row_to_edge)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(IndexError::from)
            }
        }
    }

    pub fn edges_to(&self, id: &str, kind: Option<EdgeKind>) -> Result<Vec<GraphEdge>, IndexError> {
        match kind {
            Some(k) => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, from_id, to_id, kind, confidence FROM edges WHERE to_id = ?1 AND kind = ?2",
                )?;
                let rows = stmt.query_map(params![id, k.as_str()], row_to_edge)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(IndexError::from)
            }
            None => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, from_id, to_id, kind, confidence FROM edges WHERE to_id = ?1",
                )?;
                let rows = stmt.query_map(params![id], row_to_edge)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(IndexError::from)
            }
        }
    }

    pub fn symbols_without_incoming_calls(&self) -> Result<Vec<GraphNode>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT n.id, n.kind, n.name, n.relative_path, n.parent_file, n.line, n.extension, n.language_id, n.size_bytes
             FROM nodes n
             WHERE n.kind IN ('function', 'class')
               AND NOT EXISTS (
                 SELECT 1 FROM edges e
                 WHERE e.to_id = n.id AND e.kind = 'CALLS'
               )
             ORDER BY n.relative_path, n.line",
        )?;
        let rows = stmt.query_map([], row_to_node)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(IndexError::from)
    }

    pub fn list_edges(&self) -> Result<Vec<GraphEdge>, IndexError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, from_id, to_id, kind, confidence FROM edges ORDER BY id")?;

        let rows = stmt.query_map([], row_to_edge)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(IndexError::from)
    }
}

fn row_to_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphEdge> {
    let kind_str: String = row.get(3)?;
    let kind = parse_edge_kind(&kind_str).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(3, kind_str, rusqlite::types::Type::Text)
    })?;
    let confidence: Option<f64> = row.get(4)?;
    Ok(GraphEdge {
        id: row.get(0)?,
        from_id: row.get(1)?,
        to_id: row.get(2)?,
        kind,
        confidence: confidence.map(|c| c as f32),
    })
}

fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphNode> {
    let kind_str: String = row.get(1)?;
    let kind = parse_node_kind(&kind_str).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(1, kind_str, rusqlite::types::Type::Text)
    })?;
    Ok(GraphNode {
        id: row.get(0)?,
        kind,
        name: row.get(2)?,
        relative_path: row.get(3)?,
        parent_file: row.get(4)?,
        line: row.get::<_, Option<i64>>(5)?.map(|n| n as u32),
        extension: row.get(6)?,
        language_id: row.get(7)?,
        size_bytes: row.get::<_, Option<i64>>(8)?.map(|s| s as u64),
    })
}

fn parse_node_kind(s: &str) -> Option<NodeKind> {
    match s {
        "directory" => Some(NodeKind::Directory),
        "file" => Some(NodeKind::File),
        "function" => Some(NodeKind::Function),
        "class" => Some(NodeKind::Class),
        "import" => Some(NodeKind::Import),
        "ui_element" => Some(NodeKind::UiElement),
        "route" => Some(NodeKind::Route),
        "branch" => Some(NodeKind::Branch),
        _ => None,
    }
}

fn parse_edge_kind(s: &str) -> Option<EdgeKind> {
    match s {
        "CONTAINS" => Some(EdgeKind::Contains),
        "IMPORTS" => Some(EdgeKind::Imports),
        "CALLS" => Some(EdgeKind::Calls),
        "TRIGGERS" => Some(EdgeKind::Triggers),
        "HANDLES" => Some(EdgeKind::Handles),
        "FETCHES" => Some(EdgeKind::Fetches),
        "BRANCHES_TO" => Some(EdgeKind::BranchesTo),
        _ => None,
    }
}

impl From<rusqlite::Error> for IndexError {
    fn from(value: rusqlite::Error) -> Self {
        IndexError::Sqlite(value.to_string())
    }
}
