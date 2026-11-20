param(
    [string] $WslBinary,
    [string] $Distribution
)

$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

if (-not (Get-Command codex -ErrorAction SilentlyContinue)) {
    throw 'Codex CLI is required. Install it and ensure the codex command is on PATH.'
}

Push-Location -LiteralPath $workspace
try {
    if ($WslBinary) {
        $binary = (Resolve-Path -LiteralPath $WslBinary).Path
        $wsl = (Get-Command wsl.exe).Source
        $wslArgs = @()
        if ($Distribution) {
            $wslArgs += @('--distribution', $Distribution)
        }
        $linuxBinary = & $wsl @wslArgs --exec wslpath -a $binary
        if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the WSL server path.' }
        $linuxWorkspace = & $wsl @wslArgs --exec wslpath -a $workspace
        if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the WSL workspace path.' }
        & $wsl @wslArgs --exec test -x $linuxBinary
        if ($LASTEXITCODE -ne 0) { throw 'The WSL server must be executable.' }
        & codex mcp add futty -- $wsl @wslArgs --exec $linuxBinary $linuxWorkspace
    } else {
        $cargo = Get-Command cargo -ErrorAction SilentlyContinue
        if (-not $cargo) {
            $cargo = Get-Command (Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe') -ErrorAction SilentlyContinue
        }
        if (-not $cargo) {
            throw 'Install Rust with rustup and the Visual Studio Build Tools C++ workload, then rerun this script.'
        }
        $target = Join-Path $workspace 'target'
        & $cargo.Source build --locked -p futty-mcp --target-dir $target
        if ($LASTEXITCODE -ne 0) { throw 'Futty MCP build failed. See the Cargo output above.' }
        $binary = Join-Path $target 'debug\futty-mcp.exe'
        & codex mcp add futty -- $binary $workspace
    }
    if ($LASTEXITCODE -ne 0) { throw 'Codex MCP registration failed.' }
    & codex mcp get futty
    if ($LASTEXITCODE -ne 0) { throw 'Could not verify the Futty MCP registration.' }
} finally {
    Pop-Location
}
