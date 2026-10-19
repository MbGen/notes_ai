param(
 [string]$Binary,
 [string]$LayaDirectory,
 [string]$RuntimeDirectory,
 [string]$Compiler,
 [string]$OutputDirectory
)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
foreach($file in @($Binary,$Compiler,(Join-Path $LayaDirectory 'llama-server.exe'),(Join-Path $LayaDirectory 'notes-laya-v1-f16.gguf'))) {
 if(-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Required file missing: $file" }
}
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$payload=Join-Path $OutputDirectory ('payload-'+[guid]::NewGuid())
New-Item -ItemType Directory $payload | Out-Null
Copy-Item -LiteralPath $Binary -Destination "$payload\NotesAI.exe"
foreach($name in @('msvcp140.dll','vcruntime140.dll','vcruntime140_1.dll')) {
 Copy-Item -LiteralPath (Join-Path $RuntimeDirectory $name) -Destination $payload
}
Copy-Item "$root\LICENSE" "$payload\LICENSE.txt"
Copy-Item "$root\windows\PORTABLE-README.txt" "$payload\README.txt"
New-Item -ItemType Directory "$payload\laya" | Out-Null
Get-ChildItem -LiteralPath $LayaDirectory -File | Where-Object {
 $_.Extension -eq '.dll' -or $_.Name -in @('llama-server.exe','notes-laya-v1-f16.gguf') -or $_.Name -like 'LICENSE*'
} | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination "$payload\laya" }
# Only explicitly selected runtime files enter the installer; no user database or pairing settings.
& $Compiler "/DBundleDir=$payload" "/DOutputDir=$OutputDirectory" "$root\windows\installer\notes-ai.iss"
if($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }
