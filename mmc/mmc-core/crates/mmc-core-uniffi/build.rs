use camino::Utf8PathBuf;
use std::path::PathBuf;

fn main() {
    // Get the UDL file path relative to the crate root
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let udl_path = Utf8PathBuf::from(&manifest_dir).join("src/mmc_core.udl");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let scaffolding_path = out_dir.join("mmc_core.uniffi.rs");

    println!("cargo:rerun-if-changed={}", udl_path);

    // Check if the UDL file exists
    if !PathBuf::from(udl_path.as_str()).exists() {
        println!("cargo:warning=UDL file not found at {}, generating stub", udl_path);
        generate_stub(&scaffolding_path);
        return;
    }

    // Try to generate the scaffolding
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
}

fn generate_stub(path: &PathBuf) {
    use std::io::Write;
    
    let stub = r#"// Auto-generated stub for UniFFI scaffolding
// This stub is generated when UniFFI scaffolding generation fails
// The actual bindings need to be generated separately using uniffi-bindgen

"#;

    if let Ok(mut file) = std::fs::File::create(path) {
        let _ = file.write_all(stub.as_bytes());
        println!("cargo:warning=Generated stub at {:?}", path);
    } else {
        eprintln!("cargo:warning=Failed to create stub file");
    }
}
