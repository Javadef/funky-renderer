# Download Duck glTF Model
Write-Host "🦆 Downloading Duck glTF Model..." -ForegroundColor Cyan
Write-Host ""

# Ensure models directory exists
if (!(Test-Path "models")) {
    New-Item -ItemType Directory -Path "models" | Out-Null
}

# URLs for Duck model
$duckGltfUrl = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Duck/glTF/Duck.gltf"
$duckBinUrl = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Duck/glTF/Duck0.bin"

try {
    Write-Host "📥 Downloading Duck.gltf..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $duckGltfUrl -OutFile "models/Duck.gltf"
    Write-Host "  ✓ Downloaded Duck.gltf" -ForegroundColor Green
    
    Write-Host "📥 Downloading Duck0.bin..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $duckBinUrl -OutFile "models/Duck0.bin"
    Write-Host "  ✓ Downloaded Duck0.bin" -ForegroundColor Green
    
    # Copy to scene.gltf for auto-detection
    Copy-Item "models/Duck.gltf" "models/scene.gltf" -Force
    Copy-Item "models/Duck0.bin" "models/Duck0.bin" -Force
    
    Write-Host ""
    Write-Host "✅ Duck model downloaded successfully!" -ForegroundColor Green
    Write-Host "   Files saved to models/ directory" -ForegroundColor White
    Write-Host ""
    Write-Host "🚀 Now run: cargo run --release" -ForegroundColor Cyan
}
catch {
    Write-Host ""
    Write-Host "✗ Download failed: $_" -ForegroundColor Red
    Write-Host "  Check your internet connection" -ForegroundColor Yellow
}
