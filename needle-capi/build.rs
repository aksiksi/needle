extern crate cbindgen;

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut config = cbindgen::Config::default();
    config.include_guard = Some("NEEDLE_H".to_string());
    config.language = cbindgen::Language::C;
    config.enumeration.prefix_with_name = true;

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("target/bindings.h");
}
