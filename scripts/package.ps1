param(
    [switch]$CratesIo,
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$packageProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $packageProjectRoot
$packageManifest = Get-Content -LiteralPath (Join-Path $packageProjectRoot 'Cargo.toml') -Raw
$packageVersion = [regex]::Match($packageManifest, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if (-not $packageVersion) { throw 'Cannot determine package version.' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $packageProjectRoot 'dist' }
$packageOutputRoot = [System.IO.Path]::GetFullPath($OutputDirectory)
$packageName = "LinkShop-$packageVersion-windows-x64"
$packageStage = Join-Path $packageOutputRoot $packageName
$packageZip = Join-Path $packageOutputRoot "$packageName.zip"
if ((Test-Path -LiteralPath $packageStage) -or (Test-Path -LiteralPath $packageZip)) {
    throw 'The package already exists. Choose a new -OutputDirectory to preserve the existing build.'
}

# Static MSVC CRT keeps the executable independent of a separately installed VC runtime.
$packagePreviousRustFlags = $env:RUSTFLAGS
try {
    $env:RUSTFLAGS = "$packagePreviousRustFlags -C target-feature=+crt-static".Trim()
    $packageBuildArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $PSScriptRoot 'dev.ps1'), 'release')
    if ($CratesIo) { $packageBuildArgs += '-CratesIo' }
    & powershell @packageBuildArgs
    if ($LASTEXITCODE -ne 0) { throw "Release build failed with exit code $LASTEXITCODE." }
} finally {
    $env:RUSTFLAGS = $packagePreviousRustFlags
}

$packageExecutable = Join-Path $packageProjectRoot 'target\x86_64-pc-windows-msvc\release\linkshop.exe'
if (-not (Test-Path -LiteralPath $packageExecutable)) { throw 'Release executable was not produced.' }
New-Item -ItemType Directory -Path $packageStage -Force | Out-Null
Copy-Item -LiteralPath $packageExecutable -Destination (Join-Path $packageStage 'LinkShop.exe')

$packageReadme = @"
LinkShop $packageVersion - Windows x64 免安装版

使用方法
1. 完整解压此 ZIP。
2. 双击 LinkShop.exe 启动，无需安装 Rust 或 Visual Studio。
3. 输入 http:// 或 https:// URL，点击“发送”。

常用快捷键
Ctrl+T       新建请求
Ctrl+W       关闭请求标签
Ctrl+S       保存请求
Ctrl+Enter   发送请求
Ctrl+K       搜索本地请求
Ctrl+L       聚焦 URL

请求、历史和打开的标签保存在当前用户的：
%APPDATA%\LinkShop\data\

支持参数/请求头网格、批量编辑、raw/urlencoded 请求体、Bearer/Basic/API Key、响应查看。
右键集合选择“集合身份验证”，统一设置 token；勾选批量继承可同时切换已有请求（含子文件夹）。
从集合添加的新请求默认继承；已有请求也可在“身份验证”中选择“继承集合身份验证”。更新集合 token 后，下次发送立即生效。
支持导入 Postman 集合和环境 JSON，保留文件夹层级、认证和禁用行。点击“导入”或拖入 JSON。
集合、文件夹和请求支持右键菜单，可添加、打开、复制、重命名和删除；删除前确认，已打开请求保留为草稿。
请求标签支持右键保存、复制、关闭当前/其他/右侧/全部标签；批量关闭会逐个提示保存未保存的更改。
右上角选择环境；点击“变量”编辑全局、请求和环境变量，使用 {{变量名}} 引用。
脚本执行、multipart/binary 请求体、动态变量和云协作尚未实现。

命令行导入（请先关闭所有 LinkShop 窗口）：
LinkShop.exe import "C:\exports\collection.json" "C:\exports\environment.json"
导入前自动备份现有工作区，结果保存在数据目录 last-import-report.json。

构建日期：$(Get-Date -Format 'yyyy-MM-dd')
第三方组件信息见 THIRD-PARTY-NOTICES.tsv 和 licenses 文件夹。
"@
[System.IO.File]::WriteAllText((Join-Path $packageStage '使用说明.txt'), $packageReadme, [System.Text.UTF8Encoding]::new($true))

$packageCargoConfig = @()
if (-not $CratesIo) {
    $packageCargoConfig = @('--config', "source.crates-io.replace-with='rsproxy'", '--config', "source.rsproxy.registry='sparse+https://rsproxy.cn/index/'")
}
$packageMetadataJson = & cargo @packageCargoConfig metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Could not collect third-party package metadata.' }
$packageMetadata = $packageMetadataJson | ConvertFrom-Json
$packageResolvedIds = @{}
foreach ($packageNode in $packageMetadata.resolve.nodes) { $packageResolvedIds[$packageNode.id] = $true }
$packageLicenseRoot = Join-Path $packageStage 'licenses'
New-Item -ItemType Directory -Path $packageLicenseRoot -Force | Out-Null
$packageNotices = [System.Collections.Generic.List[string]]::new()
$packageNotices.Add("Component`tVersion`tLicense`tLicense files")
foreach ($packageDependency in ($packageMetadata.packages | Where-Object { $_.source -and $packageResolvedIds.ContainsKey($_.id) } | Sort-Object name,version)) {
    $packageSourceRoot = Split-Path -Parent $packageDependency.manifest_path
    $packageLicenseFiles = @(Get-ChildItem -LiteralPath $packageSourceRoot -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|COPYRIGHT|NOTICE)([.\-_].*)?$' })
    if ($packageDependency.license_file) {
        $packageExplicitLicense = Join-Path $packageSourceRoot $packageDependency.license_file
        if (Test-Path -LiteralPath $packageExplicitLicense -PathType Leaf) { $packageLicenseFiles += Get-Item -LiteralPath $packageExplicitLicense }
    }
    $packageLicenseFiles = @($packageLicenseFiles | Sort-Object FullName -Unique)
    $packageLicenseFolder = "$($packageDependency.name)-$($packageDependency.version)"
    if ($packageLicenseFiles.Count -gt 0) {
        $packageLicenseTarget = Join-Path $packageLicenseRoot $packageLicenseFolder
        New-Item -ItemType Directory -Path $packageLicenseTarget -Force | Out-Null
        foreach ($packageLicense in $packageLicenseFiles) { Copy-Item -LiteralPath $packageLicense.FullName -Destination $packageLicenseTarget }
    }
    $packageNotices.Add("$($packageDependency.name)`t$($packageDependency.version)`t$($packageDependency.license)`t$(($packageLicenseFiles.Name | ForEach-Object { "licenses/$packageLicenseFolder/$_" }) -join '; ')")
}
[System.IO.File]::WriteAllLines((Join-Path $packageStage 'THIRD-PARTY-NOTICES.tsv'), $packageNotices, [System.Text.UTF8Encoding]::new($true))
# Cargo's packaged license files can have Unix-epoch timestamps, which ZIP cannot represent.
$packageArchiveTime = Get-Date
Get-ChildItem -LiteralPath $packageStage -Recurse -Force -File | ForEach-Object { $_.LastWriteTime = $packageArchiveTime }
Compress-Archive -LiteralPath $packageStage -DestinationPath $packageZip -CompressionLevel Optimal
$packageHash = (Get-FileHash -LiteralPath $packageZip -Algorithm SHA256).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText("$packageZip.sha256", "$packageHash  $packageName.zip`n", [System.Text.UTF8Encoding]::new($false))
Get-Item -LiteralPath $packageZip, (Join-Path $packageStage 'LinkShop.exe'), "$packageZip.sha256" | Select-Object FullName,Length
