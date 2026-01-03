fn main(){
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    
    let config = cbindgen::Config{
        language: cbindgen::Language::C,
        include_guard: Some("BIBI_SYNC_H".to_string()),
        ..Default::default()
    };

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("include/bibi_sync.h");
}