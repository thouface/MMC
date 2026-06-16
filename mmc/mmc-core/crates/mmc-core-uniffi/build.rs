use camino::Utf8PathBuf;
use std::path::PathBuf;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let udl_path = Utf8PathBuf::from(&manifest_dir).join("src/mmc_core.udl");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let scaffolding_path = out_dir.join("mmc_core.uniffi.rs");

    println!("cargo:rerun-if-changed={}", udl_path);

    if !PathBuf::from(udl_path.as_str()).exists() {
        println!("cargo:warning=UDL file not found at {}, generating stub", udl_path);
        generate_stub(&scaffolding_path);
        return;
    }

    match uniffi_bindgen::generate_component_scaffolding(&udl_path, None, false) {
        Ok(_) => {
            println!("cargo:warning=Successfully generated UniFFI scaffolding");
        }
        Err(e) => {
            println!("cargo:warning=UniFFI scaffolding generation failed: {}", e);
            println!("cargo:warning=Generating stub scaffolding");
            generate_stub(&scaffolding_path);
        }
    }

    let target = std::env::var("TARGET").unwrap();
    if target.contains("android") {
        println!("cargo:rustc-link-lib=jnigraphics");
    }
}

fn generate_stub(path: &PathBuf) {
    use std::io::Write;
    
    let stub = r#"// Auto-generated stub for UniFFI scaffolding
// This stub is generated when UniFFI scaffolding generation fails
// The actual bindings need to be generated separately using uniffi-bindgen

// Empty stub - types are already exported from lib.rs
"#;

    if let Ok(mut file) = std::fs::File::create(path) {
        let _ = file.write_all(stub.as_bytes());
        println!("cargo:warning=Generated stub at {:?}", path);
    } else {
        eprintln!("cargo:warning=Failed to create stub file");
    }
}
