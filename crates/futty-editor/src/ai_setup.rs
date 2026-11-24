use std::{
    env, fs,
    io::{self, Write},
    os::windows::process::CommandExt,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

const FILES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/ai_files.rs"));
const REGISTER: &[u8] = include_bytes!("../../../scripts/connect-mcp.ps1");
const SKILLS: &str = ".agents/skills/";

#[derive(Clone, Copy)]
pub enum Client {
    Codex,
    ClaudeCode,
    ClaudeDesktop,
}

impl Client {
    pub const ALL: [Self; 3] = [Self::Codex, Self::ClaudeCode, Self::ClaudeDesktop];

    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::ClaudeDesktop => "Claude Desktop",
        }
    }

    fn script_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "ClaudeCode",
            Self::ClaudeDesktop => "ClaudeDesktop",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "codex" => Some(Self::Codex),
            "claude-code" => Some(Self::ClaudeCode),
            "claude-desktop" => Some(Self::ClaudeDesktop),
            _ => None,
        }
    }
}

fn directory(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    let mut path = root.to_owned();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(io::Error::other("Invalid bundled path"));
        };
        path.push(part);
        let resolved = match path.canonicalize() {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
                path.canonicalize()?
            }
            result => result?,
        };
        if !resolved.starts_with(root) || !resolved.is_dir() {
            return Err(io::Error::other(format!(
                "Folder is outside the selected workspace: {}",
                path.display()
            )));
        }
        path = resolved;
    }
    Ok(path)
}

fn install_file(root: &Path, relative: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = directory(root, relative.parent().unwrap())?;
    let path = parent.join(relative.file_name().unwrap());
    match fs::symlink_metadata(&path) {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut file = tempfile::NamedTempFile::new_in(&parent)?;
    file.write_all(contents)?;
    match file.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error),
    }
}

fn stage_missing_desktop_skills(
    root: &Path,
    workspace: &Path,
    command: &mut Command,
) -> io::Result<Option<tempfile::TempDir>> {
    let zip_directory = Path::new(".futty/claude-skills");
    let existing = directory(root, zip_directory)?;
    let mut staging = None;
    for &(relative, contents) in FILES {
        let Some(skill) = relative.strip_prefix(SKILLS) else {
            continue;
        };
        let Some((name, _)) = skill.split_once('/') else {
            continue;
        };
        if existing.join(format!("{name}.zip")).try_exists()? {
            continue;
        }
        if staging.is_none() {
            let temporary = tempfile::tempdir()?;
            let source = temporary.path().canonicalize()?;
            staging = Some((temporary, source));
        }
        let (_, source) = staging.as_ref().unwrap();
        install_file(source, Path::new(skill), contents)?;
    }
    Ok(staging.map(|(temporary, _)| {
        command
            .env("FUTTY_SETUP_SKILLS", temporary.path())
            .env("FUTTY_SETUP_ZIPS", workspace.join(zip_directory));
        temporary
    }))
}

pub fn install(client: Client, workspace: &Path) -> io::Result<&'static str> {
    let workspace = std::path::absolute(workspace)?;
    let root = workspace.canonicalize()?;
    if !root.is_dir() {
        return Err(io::Error::other("Choose a project folder"));
    }
    if matches!(client, Client::ClaudeCode) {
        let config = root.join(".mcp.json");
        if let Ok(info) = fs::symlink_metadata(config)
            && !info.file_type().is_file()
        {
            return Err(io::Error::other(".mcp.json must be a regular file"));
        }
    }

    for &(relative, contents) in FILES {
        if let (Client::ClaudeCode, Some(skill)) = (client, relative.strip_prefix(SKILLS)) {
            install_file(&root, &Path::new(".claude/skills").join(skill), contents)?;
        } else {
            install_file(&root, Path::new(relative), contents)?;
        }
    }

    let windows =
        env::var_os("SystemRoot").ok_or_else(|| io::Error::other("SystemRoot is missing"))?;
    let mut command =
        Command::new(PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe"));
    command
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "try { & ([scriptblock]::Create([Console]::In.ReadToEnd())) -Client $env:FUTTY_SETUP_CLIENT -BundledEditor $env:FUTTY_SETUP_EXE -Workspace $env:FUTTY_SETUP_ROOT -SkillsSource $env:FUTTY_SETUP_SKILLS -SkillsZipDirectory $env:FUTTY_SETUP_ZIPS; exit 0 } catch { [Console]::Error.WriteLine($_.Exception.Message); exit 1 }"])
        .env_remove("PSModulePath")
        .env("FUTTY_SETUP_CLIENT", client.script_name())
        .env("FUTTY_SETUP_EXE", env::current_exe()?)
        .env("FUTTY_SETUP_ROOT", &workspace)
        .env_remove("FUTTY_SETUP_SKILLS")
        .env_remove("FUTTY_SETUP_ZIPS")
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let desktop_skills = if matches!(client, Client::ClaudeDesktop) {
        stage_missing_desktop_skills(&root, &workspace, &mut command)?
    } else {
        None
    };
    let mut child = command.spawn()?;
    let written = child.stdin.take().unwrap().write_all(REGISTER);
    let output = child.wait_with_output()?;
    drop(desktop_skills);
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(if message.trim().is_empty() {
            format!("AI setup failed ({})", output.status)
        } else {
            message.trim().to_owned()
        }));
    }
    written?;
    Ok(match client {
        Client::Codex => {
            "Futty MCP and skills are ready. Open the selected folder in a new Codex session."
        }
        Client::ClaudeCode => {
            "Open the selected folder in a new Claude Code session and approve the Futty MCP connection."
        }
        Client::ClaudeDesktop => {
            "Restart Claude Desktop. To add skills, upload the ZIPs in .futty/claude-skills from the selected folder."
        }
    })
}
