param(
    [Parameter(Mandatory)][string]$HubRoot,
    [string]$OutputDirectory,
    [string]$SdkTools,
    [string]$SigningThumbprint = '0E85D468BD75DF47E5B93DBE77C5141A2B6BCFED'
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$HubRoot = [IO.Path]::GetFullPath($HubRoot)
if (!$SdkTools) {
    $SdkTools = Join-Path $HubRoot '.tools/sdk/bin/10.0.26100.0/x64'
}
$SdkTools = [IO.Path]::GetFullPath($SdkTools)
if ($OutputDirectory) {
    $OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
}

Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $manifest = Get-Content hub/manifest.json -Raw | ConvertFrom-Json
    $appx = [xml](Get-Content hub/shell/AppxManifest.xml -Raw)
    if (!$OutputDirectory) {
        $OutputDirectory = Join-Path $PWD "artifacts/hub-release/$($manifest.version)"
    }
    $cargo = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($appx.Package.Identity.Version -ne "$($manifest.version).0" -or
        ($cargo.packages | Where-Object version -NE $manifest.version)) {
        throw 'CopyAll version mismatch'
    }

    cargo build --workspace --locked --release
    $release = Join-Path $cargo.target_directory 'release'
    $build = Join-Path (Get-Location).Path ('artifacts/hub-' + [guid]::NewGuid().ToString('N'))
    $stage = Join-Path $build 'package'
    $identity = Join-Path $build 'identity'
    New-Item -ItemType Directory -Path "$stage/worker/shell", "$stage/ui", "$identity/Assets" -Force | Out-Null

    Copy-Item "$release/copyall-worker.exe" "$stage/worker/CopyAll.Worker.exe"
    Copy-Item "$release/copyall_explorer.dll" "$stage/worker/CopyAll.Explorer.dll"
    Copy-Item hub/shell/AppxManifest.xml "$identity/AppxManifest.xml"
    Copy-Item hub/assets/Logo.png "$identity/Assets/Logo.png"
    Copy-Item hub/shell/CopyAll.cer "$stage/worker/shell/CopyAll.cer"
    $msix = Join-Path $stage 'worker/shell/CopyAll.msix'
    & (Join-Path $SdkTools 'makeappx.exe') pack /d $identity /nv /p $msix
    & (Join-Path $SdkTools 'signtool.exe') sign /fd SHA256 /s My /sha1 $SigningThumbprint $msix

    Copy-Item hub/ui/index.html, hub/ui/app.js, hub/ui/style.css "$stage/ui/"
    Copy-Item (Join-Path $HubRoot 'sdk/thodhub.js') "$stage/ui/thodhub.js"
    Copy-Item hub/manifest.json "$stage/manifest.json"
    & (Join-Path $HubRoot 'scripts/package-module.ps1') -PackageDirectory $stage `
        -Repository 'ThodProd/CopyAll' -OutputDirectory $OutputDirectory `
        -Notes 'CopyAll переписан на Rust. В ThodHUB четыре настройки меню и автозапуска. Проводник копирует полные пути без ограничений на число элементов и вложенность, а при нехватке памяти открывает TXT.'
} finally {
    Pop-Location
}
