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
    
    // Compile vertex shader
    let status = Command::new(&glslc)
        .args(&["shaders/cube.vert", "-o", "shaders/cube.vert.spv"])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("cargo:warning=Vertex shader compiled"),
        _ => println!("cargo:warning=Vertex shader compile failed - using existing .spv"),
    }
    
    // Compile fragment shader
    let status = Command::new(&glslc)
        .args(&["shaders/cube.frag", "-o", "shaders/cube.frag.spv"])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("cargo:warning=Fragment shader compiled"),
        _ => println!("cargo:warning=Fragment shader compile failed - using existing .spv"),
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
}
