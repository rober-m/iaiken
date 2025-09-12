
pub fn install_kernel() -> anyhow::Result<()> {
      use std::fs;
      use crate::messages::KernelSpec;

      // Get current executable path
      let exe_path = std::env::current_exe()?
          .to_string_lossy()
          .to_string();

      // Find Jupyter kernels directory
      let home_dir = dirs::home_dir()
          .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

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
