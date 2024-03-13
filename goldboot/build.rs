use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let dest =
        std::path::Path::new(&env::var("OUT_DIR").expect("OUT_DIR not set")).join("built.rs");
    built::write_built_file_with_opts(Some(&PathBuf::from("..")), &dest)
        .expect("Failed to acquire build-time information");
}
