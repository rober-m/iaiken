mod connection;
mod messages;

use clap::Parser;

#[derive(Parser)]
#[command(name = "aiken-kernel")]
#[command(about = "Jupyter kernel for Aiken programming language")]
pub struct Cli {
    /// Path to Jupyter connection file
    #[arg(long = "connection-file")]
    pub connection_file: Option<String>,

    /// Install kernel specification
    #[arg(long)]
    pub install: bool,

    /// Uninstall kernel specification
    #[arg(long)]
    pub uninstall: bool,
}

fn install_kernel() -> anyhow::Result<()> {
    println!("Installing Aiken kernell");
    // TODO: Create kernel.json spec file
    Ok(())
}

fn uninstall_kernel() -> anyhow::Result<()> {
    println!("uninstalling Aiken kernell");
    // TODO: Remove kernel.json spec file
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match (cli.connection_file, cli.install, cli.uninstall) {
        (Some(file), false, false) => connection::run_kernel(file).await,
        (None, true, false) => install_kernel(),
        (None, false, true) => uninstall_kernel(),
        _ => {
            eprintln!("Usage: aiken-kernel --connection-file=<file> | --install | --uninstall");
            std::process::exit(1);
        }
    }
}
