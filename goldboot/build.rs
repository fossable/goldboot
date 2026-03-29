use std::{env, path::PathBuf};

/// Download a file from `url` to `dest` if `dest` does not already exist.
fn download_if_missing(url: &str, dest: &PathBuf) {
    if dest.exists() {
        return;
    }
    eprintln!("Downloading OCR model: {url}");
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .expect("Failed to build HTTP client");
    let resp = client.get(url).send().expect("Failed to download OCR model");
    assert!(
        resp.status().is_success(),
        "Failed to download OCR model: HTTP {}",
        resp.status()
    );
    let bytes = resp.bytes().expect("Failed to read OCR model response");
    std::fs::write(dest, &bytes).expect("Failed to write OCR model");
}

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

    // Download OCR models into OUT_DIR so they can be embedded with include_bytes!
    if env::var("CARGO_FEATURE_BUILD").is_ok() {
        const HF_BASE: &str = "https://huggingface.co/robertknight/ocrs/resolve/main";
        let out = PathBuf::from(&out_dir);
        let detection = out.join("text-detection.rten");
        let recognition = out.join("text-recognition.rten");
        println!("cargo:rerun-if-changed={}", detection.display());
        println!("cargo:rerun-if-changed={}", recognition.display());
        download_if_missing(
            &format!("{HF_BASE}/text-detection-ssfbcj81.rten"),
            &detection,
        );
        download_if_missing(
            &format!("{HF_BASE}/text-rec-checkpoint-s52qdbqt.rten"),
            &recognition,
        );
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/builder/os");
    println!("cargo:rerun-if-changed=src/builder/options");
}
