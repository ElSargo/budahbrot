$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root "target\release\budahbrot.exe"

if (-not (Test-Path $exe)) {
    Push-Location $root
    try {
        cargo build --release
    } finally {
        Pop-Location
    }
}

Start-Process `
    -FilePath $exe `
    -ArgumentList @("--run-page") `
    -WorkingDirectory $root
