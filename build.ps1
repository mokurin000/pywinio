param(
    [switch]$Release,
    [switch]$Offline
)

# Cargo writes progress to stderr; a global `Stop` preference would abort the
# script even on success when the caller merges stderr into stdout. Check the
# exit code explicitly instead.
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

$profile = if ($Release) { "release" } else { "debug" }
$cargoArgs = @("build")
if ($Release) { $cargoArgs += "--release" }
if ($Offline) { $cargoArgs += "--offline" }

cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

New-Item -ItemType Directory -Force "python\pywinio" -ErrorAction Stop | Out-Null
Copy-Item "target\$profile\_pywinio.dll" "python\pywinio\_pywinio.pyd" -Force -ErrorAction Stop

Write-Host "Built python\pywinio\_pywinio.pyd ($profile)"

