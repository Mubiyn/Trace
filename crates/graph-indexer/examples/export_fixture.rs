//! Write graph JSON for a fixture: `cargo run -p graph-indexer --example export_fixture python_simple`

fn main() {
    let name = std::env::args().nth(1).expect("usage: export_fixture <fixture-name>");
    let path = graph_indexer::fixture_path(&name);
    let json = graph_indexer::export_graph_json(&path).expect("export graph");
    print!("{json}");
}
