use camino::Utf8PathBuf;
use uniffi_bindgen::bindings::KotlinBindingGenerator;
use uniffi_bindgen::generate_bindings;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut udl_path: Option<Utf8PathBuf> = None;
    let mut out_dir: Option<Utf8PathBuf> = None;
    let mut language: Option<String> = None;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "generate" => {
                i += 1;
                continue;
            }
            "--language" => {
                if i + 1 < args.len() {
                    language = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("--language requires a value");
                    std::process::exit(1);
                }
            }
            "--out-dir" => {
                if i + 1 < args.len() {
                    out_dir = Some(Utf8PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("--out-dir requires a value");
                    std::process::exit(1);
                }
            }
            _ => {
                if udl_path.is_none() {
                    udl_path = Some(Utf8PathBuf::from(&args[i]));
                }
                i += 1;
            }
        }
    }

    let udl_path = udl_path.expect("UDL file path is required");
    let out_dir = out_dir.unwrap_or_else(|| Utf8PathBuf::from("."));
    let language = language.unwrap_or_else(|| "kotlin".to_string());

    if language != "kotlin" {
        eprintln!("Only Kotlin is supported in this build");
        std::process::exit(1);
    }

    let generator = KotlinBindingGenerator;

    if let Err(e) = generate_bindings(
        &udl_path,
        None,
        generator,
        Some(out_dir.as_path()),
        None,
        None,
        false,
    ) {
        eprintln!("Error generating bindings: {}", e);
        std::process::exit(1);
    }

    println!("Generated Kotlin bindings to {}", out_dir);
}
