$mainFile = "src\main.rs"

function Stop-WithPause([int]$code) {
    Write-Host "`nPress Enter to exit..." -ForegroundColor Yellow
    [void][System.Console]::ReadLine()
    exit $code
}

# 1. Verify src/main.rs existence
if (-not (Test-Path $mainFile)) {
    Write-Host "Error: src\main.rs not found in current directory." -ForegroundColor Red
    Stop-WithPause 1
}

# 2. Run Cargo Pipeline safely without regex mutation
Write-Host "`n[1/4] Running cargo clean..." -ForegroundColor Cyan
cargo clean

Write-Host "`n[2/4] Running cargo clippy..." -ForegroundColor Cyan
cargo clippy --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nClippy check failed." -ForegroundColor Red
    Stop-WithPause $LASTEXITCODE
}

Write-Host "`n[3/4] Running cargo test..." -ForegroundColor Cyan
cargo test
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nTests failed." -ForegroundColor Red
    Stop-WithPause $LASTEXITCODE
}

Write-Host "`n[4/4] Building release binary..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nRelease build failed." -ForegroundColor Red
    Stop-WithPause $LASTEXITCODE
}

Write-Host "`n==========================================" -ForegroundColor Green
Write-Host " All checks passed & build succeeded! " -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Green
Stop-WithPause 0
