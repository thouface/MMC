fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = &["../../proto/mmc/v1/mmc.proto"];
    let out_dir = "src/generated";

    std::fs::create_dir_all(out_dir)?;

    let mut config = prost_build::Config::new();
    config
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .out_dir(out_dir);

    config.compile_protos(proto_files, &["../../proto"])?;

    println!("cargo:rerun-if-changed=../../proto/mmc/v1/mmc.proto");

    Ok(())
}
