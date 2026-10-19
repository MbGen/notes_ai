param(
    [string]$Version = '0.5.1',
    [string]$RuntimeDirectory = '',
    [string]$LayaDirectory = ''
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\release\notes_ai.exe'
if (-not (Test-Path -LiteralPath $binary)) { throw 'Build with cargo build --release --target x86_64-pc-windows-msvc first.' }
$distribution = Join-Path $projectRoot 'dist'
$staging = Join-Path $distribution ('package-' + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $staging -Force | Out-Null
Copy-Item -LiteralPath $binary -Destination (Join-Path $staging 'NotesAI.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'windows\PORTABLE-README.txt') -Destination (Join-Path $staging 'README.txt')
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination (Join-Path $staging 'LICENSE.txt')
if ($RuntimeDirectory) {
    foreach ($name in @('vcruntime140.dll','vcruntime140_1.dll','msvcp140.dll','msvcp140_1.dll','msvcp140_2.dll','msvcp140_atomic_wait.dll','msvcp140_codecvt_ids.dll')) {
        $source = Join-Path $RuntimeDirectory $name
        if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $staging $name) }
    }
}
if ($LayaDirectory) {
    foreach ($required in @('llama-server.exe','notes-laya-v1-f16.gguf')) {
        if (-not (Test-Path -LiteralPath (Join-Path $LayaDirectory $required))) { throw "Laya bundle missing $required" }
    }
    $layaTarget = Join-Path $staging 'laya'
    New-Item -ItemType Directory -Path $layaTarget | Out-Null
    Get-ChildItem -LiteralPath $LayaDirectory -File | Where-Object {
        $_.Extension -eq '.dll' -or $_.Name -in @('llama-server.exe','notes-laya-v1-f16.gguf') -or $_.Name -like 'LICENSE*'
    } | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $layaTarget $_.Name) }
}
# The staging directory is new and contains only whitelisted release files.
# Never archive the workspace or its personal data/ and settings files.
$zip = Join-Path $distribution "NotesAI-$Version-windows-x64.zip"
Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zip -Force
Copy-Item -LiteralPath $binary -Destination (Join-Path $distribution "NotesAI-$Version-windows-x64.exe")
Write-Output "ZIP: $zip"
Write-Output "Portable folder: $staging"
