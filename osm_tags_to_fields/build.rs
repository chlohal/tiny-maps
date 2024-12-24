use std::{env, fs::File, io::Write, path::Path};

use osm_schema_builder::{deprecations::make_deprecations, fields::make_fields};

fn main() {
    println!("cargo::rerun-if-changed=id-tagging-schema/");
    println!("cargo:rerun-if-changed=build.rs");

    open_file("generated_osm_structs.rs")
    .write_all(b"mod deprecations;\npub mod fields;\n").unwrap();

    make_deprecations(&mut open_file("deprecations.rs")).unwrap();
    make_fields(&mut open_file("fields.rs")).unwrap();
}

fn open_file(addr: &str) -> File {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    std::fs::File::options()
    .write(true)
    .create(true)
    .truncate(true)
    .open(Path::new(&out_dir).join(addr))
    .unwrap()
}