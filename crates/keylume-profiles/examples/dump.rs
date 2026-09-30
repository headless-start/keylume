//! Print the built-in library as JSON.
//! `cargo run -p keylume-profiles --example dump`          -> previews (id, name, colours)
//! `cargo run -p keylume-profiles --example dump -- --full` -> full profiles + layout
//! (the `--full` output, per-key colours packed as the app sends them, is the fixture
//! for the UI's browser mock mode)

fn main() {
    let layout = keylume_proto::Layout::tk68();
    let lib = keylume_profiles::builtin(&layout);
    let full = std::env::args().any(|a| a == "--full");
    let out = if full {
        let wire: Vec<_> = lib.iter().map(|p| keylume_profiles::to_wire(p, &layout)).collect();
        serde_json::json!({ "layout": layout, "profiles": wire })
    } else {
        let previews: Vec<_> = lib
            .iter()
            .map(|p| serde_json::json!({ "id": p.id, "name": p.name, "category": p.category, "preview": p.preview(&layout) }))
            .collect();
        serde_json::json!({ "layout": layout, "profiles": previews })
    };
    println!("{}", serde_json::to_string(&out).unwrap());
}
