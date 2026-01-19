use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=shaders/");
    
    // Check if Vulkan SDK is installed
    let vulkan_sdk = match std::env::var("VULKAN_SDK") {
        Ok(sdk) => sdk,
        Err(_) => {
            println!("cargo:warning=VULKAN_SDK not set - shaders must be compiled manually");
            return;
        }
    };
    
    let glslc = format!("{}\\Bin\\glslc.exe", vulkan_sdk);
    
    // Compile cube vertex shader
    let status = Command::new(&glslc)
        .args(&["shaders/cube.vert", "-o", "shaders/cube.vert.spv"])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("cargo:warning=Cube vertex shader compiled"),
        _ => println!("cargo:warning=Cube vertex shader compile failed - using existing .spv"),
    }
    
    // Compile cube fragment shader
    let status = Command::new(&glslc)
        .args(&["shaders/cube.frag", "-o", "shaders/cube.frag.spv"])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("cargo:warning=Cube fragment shader compiled"),
        _ => println!("cargo:warning=Cube fragment shader compile failed - using existing .spv"),
    }
    
    // Compile glTF vertex shader
    let status = Command::new(&glslc)
        .args(&["shaders/gltf.vert", "-o", "shaders/gltf.vert.spv"])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("cargo:warning=glTF vertex shader compiled"),
        _ => println!("cargo:warning=glTF vertex shader compile failed"),
    }
    
    // Compile glTF fragment shader
    let status = Command::new(&glslc)
        .args(&["shaders/gltf.frag", "-o", "shaders/gltf.frag.spv"])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("cargo:warning=glTF fragment shader compiled"),
        _ => println!("cargo:warning=glTF fragment shader compile failed"),
    }
    
    // Compile egui vertex shader
    let status = Command::new(&glslc)
        .args(&["shaders/egui.vert", "-o", "shaders/egui.vert.spv"])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("cargo:warning=egui vertex shader compiled"),
        _ => println!("cargo:warning=egui vertex shader compile failed"),
    }
    
    // Compile egui fragment shader
    let status = Command::new(&glslc)
        .args(&["shaders/egui.frag", "-o", "shaders/egui.frag.spv"])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("cargo:warning=egui fragment shader compiled"),
        _ => println!("cargo:warning=egui fragment shader compile failed"),
    }

    // Compile shadow map vertex shader
    let status = Command::new(&glslc)
        .args(&["shaders/shadow.vert", "-o", "shaders/shadow.vert.spv"])
        .status();

    match status {
        Ok(s) if s.success() => println!("cargo:warning=Shadow vertex shader compiled"),
        _ => println!("cargo:warning=Shadow vertex shader compile failed - using existing .spv"),
    }

    // Compile shadow map fragment shader
    let status = Command::new(&glslc)
        .args(&["shaders/shadow.frag", "-o", "shaders/shadow.frag.spv"])
        .status();

    match status {
        Ok(s) if s.success() => println!("cargo:warning=Shadow fragment shader compiled"),
        _ => println!("cargo:warning=Shadow fragment shader compile failed - using existing .spv"),
    }
}
