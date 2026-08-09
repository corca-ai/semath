use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let crate_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let packs_root = crate_root.join("../../packs");
    println!("cargo:rerun-if-changed={}", packs_root.display());

    let mut packs = fs::read_dir(&packs_root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let id = entry.file_name().into_string().ok()?;
            let source = entry.path().join("v1.json");
            source.is_file().then_some((id, source))
        })
        .collect::<Vec<_>>();
    packs.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(
        !packs.is_empty(),
        "the built-in pack catalog must not be empty"
    );

    let entries = packs
        .iter()
        .map(|(id, source)| {
            println!("cargo:rerun-if-changed={}", source.display());
            format!(
                "({id:?}, include_str!({:?})),",
                source.canonicalize().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("pack_catalog.rs"),
        format!("pub(crate) const BUILTIN_PACK_SOURCES: &[(&str, &str)] = &[\n{entries}\n];\n"),
    )
    .unwrap();
}
