param(
    [switch]$Offline
)

# Build a CPython wheel with maturin. Install it first with:
#   python -m pip install maturin

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

if (-not (Get-Command maturin -ErrorAction SilentlyContinue)) {
    throw "maturin not found. Install it with: python -m pip install maturin"
}

# Do not let the development `_pywinio.pyd` (copied by build.ps1) end up in
# the wheel next to maturin's ABI-tagged extension, and do not let it shadow
# the freshly built extension during stub introspection.
Remove-Item "python\pywinio\_pywinio*.pyd" -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force "python\pywinio\__pycache__" -ErrorAction SilentlyContinue

$commonArgs = @()
if ($Offline) { $commonArgs += "--offline" }

# Refresh the stub committed under python/pywinio, then build a wheel that
# embeds it (maturin --generate-stubs).
& maturin generate-stubs --out python --interpreter python @commonArgs
if ($LASTEXITCODE -ne 0) { throw "maturin generate-stubs failed" }

$maturinArgs = @("build", "--release") + $commonArgs

& maturin @maturinArgs
if ($LASTEXITCODE -ne 0) { throw "maturin build failed" }

$wheel = Get-ChildItem "target\wheels\*.whl" |
    Sort-Object LastWriteTime |
    Select-Object -Last 1
if ($wheel) {
    Write-Host "Wheel: $($wheel.FullName)"
}
