use std::process::{Child, Command};
use std::sync::Mutex;

struct ServerProcess(Mutex<Option<Child>>);

const SERVER_PORT: u16 = 9847;

fn spawn_candidates() -> Vec<String> {
    vec![
        "graph-server".to_string(),
        "../../target/debug/graph-server".to_string(),
        "../../../target/debug/graph-server".to_string(),
        "../../../../target/debug/graph-server".to_string(),
    ]
}

fn try_spawn_graph_server() -> Option<Child> {
    for candidate in spawn_candidates() {
        if let Ok(child) = Command::new(&candidate).spawn() {
            eprintln!("graph-desktop: started {candidate} on port {SERVER_PORT}");
            return Some(child);
        }
    }
    eprintln!("graph-desktop: could not find graph-server binary");
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let server = ServerProcess(Mutex::new(try_spawn_graph_server()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(server)
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running graph desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_server_port_is_9847() {
        assert_eq!(SERVER_PORT, 9847);
    }

    #[test]
    fn spawn_candidates_include_dev_binary() {
        let candidates = spawn_candidates();
        assert!(candidates.iter().any(|c| c.contains("target/debug")));
    }
}
