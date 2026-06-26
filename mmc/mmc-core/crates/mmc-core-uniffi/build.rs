use camino::Utf8PathBuf;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let udl_path = Utf8PathBuf::from(&manifest_dir).join("src/mmc_core.udl");

    println!("cargo:rerun-if-changed={}", udl_path);

    if !udl_path.exists() {
        println!("cargo:warning=UDL file not found at {}", udl_path);
        return;
    }

    uniffi_build::generate_scaffolding(&udl_path).unwrap();

    let target = std::env::var("TARGET").unwrap();
    if target.contains("android") {
        println!("cargo:rustc-link-lib=jnigraphics");
    }
}
