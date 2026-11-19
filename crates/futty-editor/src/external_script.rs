use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
};

use futty_runtime::MAX_SCRIPT_BYTES;
use uuid::Uuid;

use crate::Editor;

pub(crate) struct ExternalScript {
    path: PathBuf,
    seen: Vec<u8>,
}

impl ExternalScript {
    fn create(name: &str, source: &str) -> io::Result<Self> {
        let directory = std::env::temp_dir()
            .join("futty-scripts")
            .join(Uuid::new_v4().to_string());
        let name: String = name
            .chars()
            .take(60)
            .map(|ch| {
                if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let path = directory.join(format!("script-{name}.rhai"));
        fs::create_dir_all(directory)?;
        fs::write(&path, source)?;
        Ok(Self {
            path,
            seen: source.as_bytes().to_vec(),
        })
    }

    fn read_saved(&mut self) -> futty_assets::AssetResult<Option<String>> {
        let mut bytes = Vec::new();
        File::open(&self.path)?
            .take(MAX_SCRIPT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if self.seen == bytes {
            return Ok(None);
        }
        let source = futty_assets::script_source(&bytes).map(str::to_owned);
        self.seen = bytes;
        source.map(Some)
    }
}

fn open_in_code(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    let executable = ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .flat_map(|root| {
            let root = PathBuf::from(root);
            [
                root.join("Programs/Microsoft VS Code/Code.exe"),
                root.join("Microsoft VS Code/Code.exe"),
            ]
        })
        .chain(std::env::var_os("PATH").into_iter().flat_map(|paths| {
            std::env::split_paths(&paths)
                .flat_map(|directory| [directory.join("Code.exe"), directory.join("../Code.exe")])
                .collect::<Vec<_>>()
        }))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Visual Studio Code was not found")
        })?;
    #[cfg(not(windows))]
    let executable = "code";
    Command::new(executable)
        .arg("--reuse-window")
        .arg(path)
        .spawn()?;
    Ok(())
}

impl Editor {
    pub(crate) fn open_external_script(
        &mut self,
        id: Uuid,
        name: &str,
        source: &str,
    ) -> io::Result<()> {
        if let Some(script) = self.external_scripts.get(&id) {
            return open_in_code(&script.path);
        }
        let script = ExternalScript::create(name, source)?;
        open_in_code(&script.path)?;
        self.external_scripts.insert(id, script);
        Ok(())
    }

    pub(crate) fn sync_external_scripts(&mut self) {
        let mut changed = Vec::new();
        let mut failure = None;
        for asset in &self.doc.scene.assets {
            let Some(script) = self.external_scripts.get_mut(&asset.id) else {
                continue;
            };
            match script.read_saved() {
                Ok(Some(source)) => changed.push((asset.id, source)),
                Ok(None) => {}
                Err(error)
                    if error
                        .downcast_ref::<io::Error>()
                        .is_some_and(|error| error.kind() == io::ErrorKind::NotFound) => {}
                Err(error) => {
                    failure = Some(format!("{}: {error}. VS Code draft kept.", asset.name))
                }
            }
        }
        for (id, source) in changed {
            self.apply_external_source(id, source);
        }
        if let Some(error) = failure {
            self.report(error, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_new_valid_saves_without_discarding_the_file() -> io::Result<()> {
        let mut script = ExternalScript::create("../CON", "let before = 1;")?;
        assert!(
            script
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("script-")
        );
        assert!(script.read_saved().unwrap().is_none());
        fs::write(&script.path, "let after = 2;")?;
        assert_eq!(
            script.read_saved().unwrap().as_deref(),
            Some("let after = 2;")
        );
        assert!(script.read_saved().unwrap().is_none());
        fs::write(&script.path, [0xff])?;
        assert!(script.read_saved().is_err());
        assert_eq!(fs::read(&script.path)?, [0xff]);
        fs::remove_file(&script.path)?;
        fs::remove_dir(script.path.parent().unwrap())
    }
}
