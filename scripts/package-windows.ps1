# Build a portable Windows x64 ZIP with GUI and CLI executables.
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Push-Location $Root
try {
    $Target = 'x86_64-pc-windows-msvc'
    cargo build --locked --release --target $Target -p hostmask-gui -p hostmask
    if ($LASTEXITCODE -ne 0) { throw 'Windows build failed' }
    $TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
    $Stage = Join-Path $Root 'dist/Hostmask-windows-x86_64'
    New-Item -ItemType Directory -Force -Path $Stage | Out-Null
    Copy-Item (Join-Path $TargetDir "$Target/release/hostmask-gui.exe") (Join-Path $Stage 'Hostmask.exe')
    Copy-Item (Join-Path $TargetDir "$Target/release/hostmask.exe") (Join-Path $Stage 'hostmask-cli.exe')
    Copy-Item (Join-Path $Root 'windows/README.txt') (Join-Path $Stage 'README.txt')
    Compress-Archive -Path $Stage -DestinationPath (Join-Path $Root 'dist/Hostmask-windows-x86_64.zip') -Force
} finally {
    Pop-Location
}
