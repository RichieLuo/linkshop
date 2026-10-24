param(
    [ValidateSet('run', 'check', 'build', 'test', 'clippy', 'release')]
    [string]$Task = 'run',
    [switch]$CratesIo
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $projectRoot

# The machine has a newer but incomplete Visual Studio preview first on PATH.
# Put the installed VS 2022 Build Tools and Windows SDK ahead of it for Rust.
$vsRoot = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools'
$vcToolsRoot = Join-Path $vsRoot 'VC\Tools\MSVC'
$vcTools = Get-ChildItem -LiteralPath $vcToolsRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
$kitsRoot = 'C:\Program Files (x86)\Windows Kits\10'
$sdkVersion = Get-ChildItem -LiteralPath (Join-Path $kitsRoot 'Lib') -Directory | Sort-Object Name -Descending | Select-Object -First 1
if ($vcTools -and $sdkVersion) {
    $vcLib = Join-Path $vcTools.FullName 'lib\x64'
    $sdkLib = Join-Path $kitsRoot "Lib\$($sdkVersion.Name)"
    $env:LIB = @($vcLib, (Join-Path $sdkLib 'um\x64'), (Join-Path $sdkLib 'ucrt\x64')) -join ';'
    $env:INCLUDE = @(
        (Join-Path $vcTools.FullName 'include'),
        (Join-Path $kitsRoot "Include\$($sdkVersion.Name)\ucrt"),
        (Join-Path $kitsRoot "Include\$($sdkVersion.Name)\shared"),
        (Join-Path $kitsRoot "Include\$($sdkVersion.Name)\um"),
        (Join-Path $kitsRoot "Include\$($sdkVersion.Name)\winrt")
    ) -join ';'
    $env:PATH = (Join-Path $vcTools.FullName 'bin\HostX64\x64') + ';' + $env:PATH
}
$cargoConfig = @()
if (-not $CratesIo) {
    $cargoConfig = @(
        '--config', "source.crates-io.replace-with='rsproxy'",
        '--config', "source.rsproxy.registry='sparse+https://rsproxy.cn/index/'"
    )
}
switch ($Task) {
    'run'   { & cargo @cargoConfig run --locked }
    'check' { & cargo @cargoConfig check --locked }
    'build' { & cargo @cargoConfig build --locked }
    'release' { & cargo @cargoConfig build --release --locked --target x86_64-pc-windows-msvc }
    'test'  { & cargo @cargoConfig test --locked }
    'clippy' { & cargo clippy @cargoConfig --locked --all-targets -- -D warnings }
}
exit $LASTEXITCODE


