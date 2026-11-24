use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn collect(path: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let kind = fs::symlink_metadata(path)?.file_type();
    if kind.is_dir() {
        println!("cargo:rerun-if-changed={}", path.display());
        for entry in fs::read_dir(path)? {
            collect(&entry?.path(), files)?;
        }
    } else if kind.is_file() {
        files.push(path.to_owned());
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = crate_dir.join("../..");
    let mut files = Vec::new();
    for path in [
        ".agents/skills",
        "docs/guide.md",
        "docs/FUTTY_FINAL_GOAL.md",
        "docs/game-skills.md",
        "docs/failure-report.md",
        "examples/agent-escape",
        "scripts/generate-prototype-assets.py",
    ] {
        collect(&root.join(path), &mut files)?;
    }
    files.sort();
    let mut source = String::from("&[\n");
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let portable = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        source.push_str(&format!("    ({portable:?}, include_bytes!({path:?})),\n"));
    }
    source.push_str("]\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("ai_files.rs"),
        source,
    )
}
