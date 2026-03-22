use std::{env, path::PathBuf};

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");

    // Generate built.rs
    if built::write_built_file().is_err() {
        let dest = std::path::Path::new(&out_dir).join("built.rs");
        built::write_built_file_with_opts(Some(&PathBuf::from("..")), &dest)
            .expect("Failed to acquire build-time information");
    }

    // Generate rust_analyzer.json for the LSP
    let workspace_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let mut analyzer = roniker::RustAnalyzer::new();

    for entry in walkdir::WalkDir::new(&workspace_root.join("src/builder/os"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
    {
        let _ = analyzer.add_file(entry.path());
    }

    for entry in walkdir::WalkDir::new(&workspace_root.join("src/builder/options"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
    {
        let _ = analyzer.add_file(entry.path());
    }

    let json = serde_json::to_string(&analyzer).expect("Failed to serialize RustAnalyzer");
    let dest = PathBuf::from(&out_dir).join("rust_analyzer.json");
    std::fs::write(&dest, json).expect("Failed to write rust_analyzer.json");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/builder/os");
    println!("cargo:rerun-if-changed=src/builder/options");
}
