param(
    [string] $WslBinary,
    [string] $Distribution,
    [ValidateSet('Codex', 'ClaudeCode', 'ClaudeDesktop')]
    [string] $Client = 'Codex',
    [string] $BundledEditor,
    [string] $Workspace,
    [string] $SkillsSource,
    [string] $SkillsZipDirectory
)

$ErrorActionPreference = 'Stop'
$workspace = if ($BundledEditor) {
    (Resolve-Path -LiteralPath $Workspace).Path
} else {
    (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
}

$cliArgs = @()
if ($Client -eq 'Codex' -or ($Client -eq 'ClaudeCode' -and !$BundledEditor)) {
    $cli = if ($Client -eq 'ClaudeCode') { 'claude' } else { 'codex' }
    if ($BundledEditor) {
        $cli = (Get-Command codex.exe -CommandType Application -ErrorAction SilentlyContinue).Source
        if (!$cli) {
            $launcher = (Get-Command codex.cmd -CommandType Application -ErrorAction SilentlyContinue).Source
            $node = (Get-Command node.exe -CommandType Application -ErrorAction SilentlyContinue).Source
            if ($launcher -and $node) {
                $entryPoint = Join-Path (Split-Path -Parent $launcher) 'node_modules\@openai\codex\bin\codex.js'
                if (Test-Path -LiteralPath $entryPoint -PathType Leaf) {
                    $cli = $node
                    $cliArgs = @($entryPoint)
                }
            }
        }
    }
    if (!$cli -or -not (Get-Command $cli -ErrorAction SilentlyContinue)) {
        throw "The $Client CLI is required. Install it and make sure it is on PATH."
    }
} elseif ($Client -eq 'ClaudeDesktop') {
    $configFolder = Join-Path $env:APPDATA 'Claude'
    if (-not (Test-Path -LiteralPath $configFolder -PathType Container)) {
        throw 'Open Claude Desktop once before registering a local MCP server.'
    }
}

function Set-JsonMcp([string] $ConfigPath, [string] $Executable, [string[]] $Arguments) {
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
    $settings = if ($entry -and $entry.Value -is [pscustomobject]) {
        $entry.Value
    } else {
        [pscustomobject]@{}
    }
    $sameRegistration = $settings.command -is [string] -and $settings.command -ceq $Executable -and
        $settings.args -is [array] -and $settings.args.Count -eq $Arguments.Count -and
        (($Client -ne 'ClaudeCode' -and !$settings.PSObject.Properties['type']) -or $settings.type -eq 'stdio')
    if ($sameRegistration) {
        for ($i = 0; $i -lt $Arguments.Count; $i++) {
            if ($settings.args[$i] -cne $Arguments[$i]) {
                $sameRegistration = $false
                break
            }
        }
        if ($sameRegistration) { return }
    }
    if ($Client -eq 'ClaudeCode' -or $settings.PSObject.Properties['type']) {
        $settings | Add-Member -NotePropertyName type -NotePropertyValue 'stdio' -Force
        $settings.PSObject.Properties.Remove('url')
        $settings.PSObject.Properties.Remove('headers')
    }
    $settings | Add-Member -NotePropertyName command -NotePropertyValue $Executable -Force
    $settings | Add-Member -NotePropertyName args -NotePropertyValue $Arguments -Force
    $config.mcpServers | Add-Member -NotePropertyName futty -NotePropertyValue $settings -Force
    $temporary = Join-Path (Split-Path -Parent $configPath) ([guid]::NewGuid().ToString() + '.tmp')
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
    Write-Host "Registered futty in $configPath."
}

Push-Location -LiteralPath $workspace
try {
    if ($BundledEditor) {
        $executable = (Resolve-Path -LiteralPath $BundledEditor).Path
        $serverArgs = @('--mcp', $workspace)
    } elseif ($WslBinary) {
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
        Set-JsonMcp (Join-Path $configFolder 'claude_desktop_config.json') $executable $serverArgs
        if ($SkillsSource -and $SkillsZipDirectory) {
            Add-Type -AssemblyName System.IO.Compression.FileSystem
            foreach ($skill in Get-ChildItem -LiteralPath $SkillsSource -Directory) {
                $zip = Join-Path $SkillsZipDirectory ($skill.Name + '.zip')
                if (Test-Path -LiteralPath $zip) { continue }
                $temporary = Join-Path $SkillsZipDirectory ([guid]::NewGuid().ToString() + '.zip')
                try {
                    [IO.Compression.ZipFile]::CreateFromDirectory(
                        $skill.FullName, $temporary, [IO.Compression.CompressionLevel]::Optimal, $true
                    )
                    [IO.File]::Move($temporary, $zip)
                } finally {
                    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
                }
            }
        }
        Write-Host 'Fully quit and reopen Claude Desktop to load it.'
    } elseif ($BundledEditor -and $Client -eq 'ClaudeCode') {
        Set-JsonMcp (Join-Path $workspace '.mcp.json') $executable $serverArgs
    } else {
        if ($Client -eq 'ClaudeCode') {
            & $cli mcp add --scope local --transport stdio futty -- $executable @serverArgs
        } else {
            & $cli @cliArgs mcp add futty -- $executable @serverArgs
        }
        if ($LASTEXITCODE -ne 0) { throw "$Client MCP registration failed." }
        & $cli @cliArgs mcp get futty
        if ($LASTEXITCODE -ne 0) { throw 'Could not verify the Futty MCP registration.' }
    }
} finally {
    Pop-Location
}
