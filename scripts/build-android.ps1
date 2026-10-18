param(
    [string[]]$Tasks = @('assembleDebug'),
    [string]$ToolRoot = ''
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (-not $ToolRoot) { $ToolRoot = Join-Path (Split-Path -Parent $projectRoot) '.notes-ai-tools\android-build' }
$ToolRoot = [IO.Path]::GetFullPath($ToolRoot)
if ([IO.Path]::GetPathRoot($ToolRoot) -ine 'D:\') { throw 'Use the Android tool directory on drive D.' }
$jdk = Get-ChildItem (Join-Path $ToolRoot 'tools') -Directory -Filter 'jdk-17*' | Select-Object -First 1
if (-not $jdk) { throw "JDK 17 was not found in $ToolRoot\tools" }
$env:JAVA_HOME = $jdk.FullName
$env:ANDROID_HOME = Join-Path $ToolRoot 'sdk'
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:GRADLE_USER_HOME = Join-Path $ToolRoot 'gradle-cache'
$env:ANDROID_USER_HOME = Join-Path $ToolRoot 'user-home\.android'
$env:ANDROID_AVD_HOME = Join-Path $ToolRoot 'user-home\avd'
$env:ANDROID_EMULATOR_HOME = Join-Path $ToolRoot 'user-home\emulator'
$env:TEMP = Join-Path $ToolRoot 'tmp'
$env:TMP = $env:TEMP
$env:TMPDIR = $env:TEMP
$taskUserHome = Join-Path $ToolRoot 'user-home'
foreach ($folder in @($env:TEMP,$env:GRADLE_USER_HOME,$env:ANDROID_USER_HOME,$env:ANDROID_AVD_HOME,$env:ANDROID_EMULATOR_HOME)) {
    New-Item -ItemType Directory -Force -Path $folder | Out-Null
}
$env:JAVA_TOOL_OPTIONS = "-Duser.home=$taskUserHome -Djava.io.tmpdir=$($env:TEMP) -Dfile.encoding=UTF-8"
$env:PATH = "$($env:JAVA_HOME)\bin;$($env:ANDROID_HOME)\platform-tools;$env:PATH"
$sdkPath = $env:ANDROID_HOME.Replace('\','/')
Set-Content -LiteralPath (Join-Path $projectRoot 'android\local.properties') -Value "sdk.dir=$sdkPath" -Encoding ascii
$gradle = Join-Path $ToolRoot 'tools\gradle-8.11.1\bin\gradle.bat'
Push-Location (Join-Path $projectRoot 'android')
try {
    & $gradle --no-daemon --console plain --project-cache-dir (Join-Path $ToolRoot 'project-cache') @Tasks
    $buildExitCode=$LASTEXITCODE
} finally { Pop-Location }
if ($buildExitCode -ne 0) { throw "Android build failed (exit $buildExitCode)." }

if ($Tasks -contains 'assembleDebug') {
    $artifact = Join-Path $projectRoot 'android\app\build\outputs\apk\debug\app-debug.apk'
    $metadata = Get-Content -Raw (Join-Path $projectRoot 'android\app\build\outputs\apk\debug\output-metadata.json') | ConvertFrom-Json
    $version = $metadata.elements[0].versionName
    $destination = Join-Path $projectRoot "dist\NotesAI-$version-debug.apk"
    New-Item -ItemType Directory -Force (Split-Path -Parent $destination) | Out-Null
    Copy-Item -LiteralPath $artifact -Destination $destination
    Write-Output "APK: $destination"
}
