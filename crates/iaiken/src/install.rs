use serde::{Deserialize, Serialize};

// Kernel specification for installation
// DOCS: https://jupyter-client.readthedocs.io/en/latest/kernels.html#kernel-specs
#[derive(Serialize, Deserialize, Debug)]
pub struct KernelSpec {
    pub argv: Vec<String>, // A list of command line arguments used to start the kernel
    pub display_name: String, // The kernel’s name as it should be displayed in the UI
    pub language: String,  // The name of the language of the kernel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>, // A dictionary of environment variables to set for the kernel
}

impl KernelSpec {
    pub fn new(executable_path: &str) -> Self {
        Self {
            argv: vec![
                executable_path.to_string(),
                "--connection-file".to_string(),
                "{connection_file}".to_string(),
            ],
            display_name: "Aiken".to_string(),
            language: "aiken".to_string(),
            env: None,
        }
    }
}

pub fn install_kernel() -> anyhow::Result<()> {
    use std::fs;

    // Get current executable path
    let exe_path = std::env::current_exe()?.to_string_lossy().to_string();

    // Find Jupyter kernels directory
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    let kernels_dir = home_dir
        .join(".local")
        .join("share")
        .join("jupyter")
        .join("kernels")
        .join("aiken");

    // Create directory if it doesn't exist
    fs::create_dir_all(&kernels_dir)?;

    // Create kernel spec
    let spec = KernelSpec::new(&exe_path);

    // Write kernel.json
    let kernel_json_path = kernels_dir.join("kernel.json");
    let spec_json = serde_json::to_string_pretty(&spec)?;
    fs::write(&kernel_json_path, spec_json)?;

    println!("Aiken kernel installed successfully!");
    println!("Kernel spec written to: {}", kernel_json_path.display());

    Ok(())
}

pub fn uninstall_kernel() -> anyhow::Result<()> {
    println!("uninstalling Aiken kernell");
    // TODO: Remove kernel.json spec file
    Ok(())
}
