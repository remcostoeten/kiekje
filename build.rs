#[path = "build/css.rs"]
mod css;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-changed=resources/style.css");
    println!("cargo:rerun-if-changed=build/css.rs");
    let source =
        fs::read_to_string("resources/style.css").expect("resources/style.css is readable");
    let flat = css::flatten(&source).unwrap_or_else(|error| panic!("style.css: {error}"));
    let generated = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("css");
    fs::create_dir_all(&generated).expect("OUT_DIR is writable");
    fs::write(generated.join("style.css"), flat).expect("OUT_DIR is writable");
    glib_build_tools::compile_resources(
        &[generated.as_path(), Path::new("resources")],
        "resources/resources.gresource.xml",
        "kiekje.gresource",
    );
}
