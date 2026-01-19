# Download Sample glTF Models
# This script downloads popular sample models for testing

Write-Host "🎨 glTF Sample Model Downloader" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""

# Ensure models directory exists
if (!(Test-Path "models")) {
    New-Item -ItemType Directory -Path "models" | Out-Null
    Write-Host "✓ Created models/ directory" -ForegroundColor Green
}

Write-Host "Available sample models:" -ForegroundColor Yellow
Write-Host ""
Write-Host "1. Duck - Classic glTF test model (simple)" -ForegroundColor White
Write-Host "2. Box - Textured box (simple)" -ForegroundColor White  
Write-Host "3. Avocado - Detailed fruit model (medium)" -ForegroundColor White
Write-Host "4. DamagedHelmet - PBR showcase (complex)" -ForegroundColor White
Write-Host "5. Sponza - Architectural scene (large)" -ForegroundColor White
Write-Host ""
Write-Host "Or use the included rainbow cube: models/scene.gltf" -ForegroundColor Cyan
Write-Host ""

$choice = Read-Host "Enter number (1-5) or press Enter to skip"

$models = @{
    "1" = @{
        name = "Duck"
        url = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Duck/glTF/Duck.gltf"
        bin = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Duck/glTF/Duck0.bin"
    }
    "2" = @{
        name = "Box"
        url = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Box/glTF/Box.gltf"
        bin = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Box/glTF/Box0.bin"
    }
    "3" = @{
        name = "Avocado"
        url = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Avocado/glTF/Avocado.gltf"
        bin = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Avocado/glTF/Avocado.bin"
    }
    "4" = @{
        name = "DamagedHelmet"
        url = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/DamagedHelmet/glTF/DamagedHelmet.gltf"
        bin = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/DamagedHelmet/glTF/DamagedHelmet.bin"
    }
    "5" = @{
        name = "Sponza"
        url = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Sponza/glTF/Sponza.gltf"
        bin = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Models/master/2.0/Sponza/glTF/Sponza.bin"
    }
}

if ($models.ContainsKey($choice)) {
    $model = $models[$choice]
    $name = $model.name
    
    Write-Host ""
    Write-Host "📥 Downloading $name..." -ForegroundColor Yellow
    
    try {
        # Download glTF file
        $gltfPath = "models/$name.gltf"
        Invoke-WebRequest -Uri $model.url -OutFile $gltfPath
        Write-Host "  ✓ Downloaded $name.gltf" -ForegroundColor Green
        
        # Download bin file
        $binPath = "models/$name.bin"
        Invoke-WebRequest -Uri $model.bin -OutFile $binPath
        Write-Host "  ✓ Downloaded $name.bin" -ForegroundColor Green
        
        # Rename to scene.gltf so it's auto-detected
        if (Test-Path "models/scene.gltf") {
            Write-Host ""
            $overwrite = Read-Host "Overwrite existing scene.gltf? (y/n)"
            if ($overwrite -eq "y") {
                Copy-Item $gltfPath "models/scene.gltf" -Force
                Copy-Item $binPath "models/scene.bin" -Force
                Write-Host "  ✓ Set as active model (scene.gltf)" -ForegroundColor Green
            }
        } else {
            Copy-Item $gltfPath "models/scene.gltf"
            Copy-Item $binPath "models/scene.bin"
            Write-Host "  ✓ Set as active model (scene.gltf)" -ForegroundColor Green
        }
        
        Write-Host ""
        Write-Host "✅ Download complete!" -ForegroundColor Green
        Write-Host "   Run 'cargo run --release' to see the model" -ForegroundColor Cyan
    }
    catch {
        Write-Host "  ✗ Download failed: $_" -ForegroundColor Red
    }
} else {
    Write-Host "Using existing model files" -ForegroundColor Green
}

Write-Host ""
Write-Host "💡 Tips:" -ForegroundColor Yellow
Write-Host "  - Models load from models/scene.gltf or models/model.gltf" -ForegroundColor White
Write-Host "  - Get more at: https://github.com/KhronosGroup/glTF-Sample-Models" -ForegroundColor White
Write-Host "  - Export from Blender: File → Export → glTF 2.0" -ForegroundColor White
