param([Parameter(Mandatory=$true)][string]$HubRoot,[string]$OutputDirectory='artifacts/hub-release',
 [string]$Compiler,[string]$SdkTools,[string]$SigningThumbprint='0E85D468BD75DF47E5B93DBE77C5141A2B6BCFED')
$ErrorActionPreference='Stop'
$PSNativeCommandUseErrorActionPreference=$true
$HubRoot=[IO.Path]::GetFullPath($HubRoot)
if(!$Compiler){$Compiler=Join-Path $HubRoot '.tools/llvm/llvm-mingw-20260616-ucrt-x86_64/bin/clang++.exe'}
if(!$SdkTools){$SdkTools=Join-Path $HubRoot '.tools/sdk/bin/10.0.26100.0/x64'}
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $stage=Join-Path (Get-Location).Path ('artifacts/hub-'+[guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    dotnet publish hub/CopyAll.Worker.csproj -c Release -r win-x64 --self-contained true -o (Join-Path $stage "worker") --nologo
    if ($LASTEXITCODE) { throw "Build failed" }
    & $Compiler -shared -O2 -std=c++17 -static -static-libgcc -static-libstdc++ hub/shell/Command.cpp hub/shell/exports.def -o (Join-Path $stage 'worker/CopyAll.Explorer.dll') -lole32 -lshell32 -lshlwapi -luuid
    $identity=Join-Path $stage 'identity-build'
    New-Item -ItemType Directory -Path "$identity/Assets","$stage/worker/shell" -Force | Out-Null
    Copy-Item hub/shell/AppxManifest.xml "$identity/AppxManifest.xml"
    Copy-Item hub/assets/Logo.png "$identity/Assets/Logo.png"
    & (Join-Path $SdkTools 'makeappx.exe') pack /o /d $identity /nv /p "$stage/worker/shell/CopyAll.msix"
    & (Join-Path $SdkTools 'signtool.exe') sign /fd SHA256 /s My /sha1 $SigningThumbprint "$stage/worker/shell/CopyAll.msix"
    Copy-Item hub/shell/CopyAll.cer "$stage/worker/shell/CopyAll.cer"
    $resolvedIdentity=[IO.Path]::GetFullPath($identity)
    if(!$resolvedIdentity.StartsWith([IO.Path]::GetFullPath($stage)+[IO.Path]::DirectorySeparatorChar)){throw 'Unsafe identity-build directory'}
    Remove-Item -LiteralPath $resolvedIdentity -Recurse -Force
    Copy-Item hub/ui (Join-Path $stage "ui") -Recurse
    Copy-Item (Join-Path $HubRoot "sdk/thodhub.js") (Join-Path $stage "ui/thodhub.js")
    Copy-Item hub/manifest.json (Join-Path $stage 'manifest.json')
    & (Join-Path $HubRoot 'scripts/package-module.ps1') -PackageDirectory $stage -Repository 'ThodProd/CopyAll' -OutputDirectory $OutputDirectory -Notes 'Настоящее расширение Проводника: CopyAll в первом меню Windows 11 и классическом меню Windows 10. Копирование путей папок и файлов работает без хаба. Добавлена собственная иконка. На новом компьютере установка расширения запрашивает доверие сертификату через UAC; обновления используют сохранённое доверие.'
} finally { Pop-Location }
