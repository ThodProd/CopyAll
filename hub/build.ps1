param([Parameter(Mandatory=$true)][string]$HubRoot,[string]$OutputDirectory='artifacts/hub-release')
$ErrorActionPreference='Stop'
$HubRoot=[IO.Path]::GetFullPath($HubRoot)
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $stage=Join-Path (Get-Location).Path ('artifacts/hub-'+[guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    dotnet publish hub/CopyAll.Worker.csproj -c Release -r win-x64 --self-contained true -o (Join-Path $stage "worker") --nologo
    if ($LASTEXITCODE) { throw "Build failed" }
    Copy-Item hub/ui (Join-Path $stage "ui") -Recurse
    Copy-Item (Join-Path $HubRoot "sdk/thodhub.js") (Join-Path $stage "ui/thodhub.js")
    Copy-Item hub/manifest.json (Join-Path $stage 'manifest.json')
    & (Join-Path $HubRoot 'scripts/package-module.ps1') -PackageDirectory $stage -Repository 'ThodProd/CopyAll' -OutputDirectory $OutputDirectory -Notes 'Встроенный интерфейс, сканирование папок, копирование и экспорт; полноэкранный режим.'
} finally { Pop-Location }
