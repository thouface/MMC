use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let scaffolding_path = out_dir.join("mmc_core.uniffi.rs");

    generate_stub(&scaffolding_path);

    let target = std::env::var("TARGET").unwrap();
    if target.contains("android") {
        println!("cargo:rustc-link-lib=jnigraphics");
    }
}

fn generate_stub(path: &std::path::Path) {
    use std::io::Write;

    let stub = "// UniFFI scaffolding stub\n\
// Full scaffolding is temporarily disabled due to type mismatches\n\
// between UDL definitions and existing Rust types.\n\
// Kotlin bindings are generated separately via uniffi-bindgen CLI.\n";

    if let Ok(mut file) = std::fs::File::create(path) {
        let _ = file.write_all(stub.as_bytes());
        println!("cargo:warning=Generated UniFFI stub scaffolding");
    }
}
