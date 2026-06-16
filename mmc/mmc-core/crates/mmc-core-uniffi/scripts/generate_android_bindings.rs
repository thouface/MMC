//! Script to generate Android bindings using UniFFI

use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| "./".to_string());
    let udl_path = Path::new(&manifest_dir).join("src/mmc_core.udl");
    let out_dir = Path::new(&manifest_dir).join("bindings/android");
    
    std::fs::create_dir_all(&out_dir)?;
    
    println!("Generating Android bindings from {:?} to {:?}", udl_path, out_dir);
    
    uniffi_bindgen::generate_bindings(
        &udl_path,
        uniffi_bindgen::Language::Kotlin,
        Some(&out_dir),
        None,
        None,
        false,
        false,
    )?;
    
    println!("Successfully generated Android bindings");
    Ok(())
}
