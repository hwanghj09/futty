param(
    [string] $WslBinary,
    [string] $Distribution,
    [ValidateSet('Codex', 'ClaudeCode', 'ClaudeDesktop')]
    [string] $Client = 'Codex'
)

$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

if ($Client -ne 'ClaudeDesktop') {
    $cli = if ($Client -eq 'ClaudeCode') { 'claude' } else { 'codex' }
    if (-not (Get-Command $cli -ErrorAction SilentlyContinue)) {
        throw "The $cli CLI is required. Install it and make sure it is on PATH."
    }
} else {
    $configFolder = Join-Path $env:APPDATA 'Claude'
    if (-not (Test-Path -LiteralPath $configFolder -PathType Container)) {
        throw 'Open Claude Desktop once before registering a local MCP server.'
    }
}

function Set-ClaudeDesktopMcp([string] $Executable, [string[]] $Arguments) {
    $configPath = Join-Path $configFolder 'claude_desktop_config.json'
    $exists = Test-Path -LiteralPath $configPath
    $config = if ($exists) {
        Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    } else {
        [pscustomobject]@{}
    }
    if ($config -isnot [pscustomobject]) {
        throw 'Claude Desktop configuration must be a JSON object.'
    }
    if ($null -eq $config.mcpServers) {
        $config | Add-Member -NotePropertyName mcpServers -NotePropertyValue ([pscustomobject]@{}) -Force
    }
    if ($config.mcpServers -isnot [pscustomobject]) {
        throw 'Claude Desktop mcpServers must be a JSON object.'
    }
    $entry = $config.mcpServers.PSObject.Properties['futty']
    if ($entry -and $entry.Value -is [pscustomobject]) {
        $entry.Value | Add-Member -NotePropertyName command -NotePropertyValue $Executable -Force
        $entry.Value | Add-Member -NotePropertyName args -NotePropertyValue $Arguments -Force
    } else {
        $config.mcpServers | Add-Member -NotePropertyName futty -NotePropertyValue @{
            command = $Executable
            args = $Arguments
        } -Force
    }
    $temporary = Join-Path $configFolder ([guid]::NewGuid().ToString() + '.tmp')
    try {
        $json = $config | ConvertTo-Json -Depth 100
        [IO.File]::WriteAllText($temporary, $json, [Text.UTF8Encoding]::new($false))
        if ($exists) {
            [IO.File]::Replace($temporary, $configPath, [NullString]::Value)
        } else {
            [IO.File]::Move($temporary, $configPath)
        }
    } finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
    }
    Write-Host "Registered futty in $configPath. Fully quit and reopen Claude Desktop to load it."
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
        $executable = $wsl
        $serverArgs = $wslArgs + @('--exec', $linuxBinary, $linuxWorkspace)
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
        $executable = Join-Path $target 'debug\futty-mcp.exe'
        $serverArgs = @($workspace)
    }
    if ($Client -eq 'ClaudeDesktop') {
        Set-ClaudeDesktopMcp $executable $serverArgs
    } else {
        if ($Client -eq 'ClaudeCode') {
            & $cli mcp add --scope local --transport stdio futty -- $executable @serverArgs
        } else {
            & $cli mcp add futty -- $executable @serverArgs
        }
        if ($LASTEXITCODE -ne 0) { throw "$Client MCP registration failed." }
        & $cli mcp get futty
        if ($LASTEXITCODE -ne 0) { throw 'Could not verify the Futty MCP registration.' }
    }
} finally {
    Pop-Location
}
