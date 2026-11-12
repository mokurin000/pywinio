param(
    [switch]$Release,
    [switch]$Offline
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

$profile = if ($Release) { "release" } else { "debug" }
$cargoArgs = @("build")
if ($Release) { $cargoArgs += "--release" }
if ($Offline) { $cargoArgs += "--offline" }

cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

New-Item -ItemType Directory -Force "python\pywinio" | Out-Null
Copy-Item "target\$profile\_pywinio.dll" "python\pywinio\_pywinio.pyd" -Force

Write-Host "Built python\pywinio\_pywinio.pyd ($profile)"
