use clap::{Parser, Subcommand};
use pluvia_cli::client::{default_socket_path, PluviaClient};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pluvia-cli", about = "Pluvia desktop engine CLI controller")]
struct Cli {
    /// Custom UNIX domain socket path
    #[arg(short, long)]
    socket: Option<PathBuf>,

    /// Format output as JSON
    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Ping the running Pluvia daemon
    Ping,

    /// Load a Rainmeter skin .ini file
    Load {
        /// Path to the skin .ini file
        path: PathBuf,
    },

    /// Unload an active skin instance by ID
    Unload {
        /// ID of the skin to unload
        id: String,
    },

    /// List all currently loaded skins
    List,

    /// Query daemon status and uptime
    Status,

    /// Refresh a specific skin or all active skins
    Refresh {
        /// Optional skin ID to refresh (refreshes all if omitted)
        id: Option<String>,
    },

    /// Dynamically update a skin variable
    SetVar {
        /// Target skin ID
        id: String,
        /// Variable name
        key: String,
        /// New variable value
        value: String,
    },

    /// Extract and install a .rmskin package archive
    Import {
        /// Path to the .rmskin or .zip package archive
        archive_path: PathBuf,
        /// Optional custom destination skins directory
        #[arg(short, long)]
        dest: Option<PathBuf>,
    },

    /// Package a skin directory into a .rmskin archive
    Pack {
        /// Path to the source skin directory
        source: PathBuf,
        /// Destination path for the .rmskin archive
        output: PathBuf,
    },

    /// Start or manage the Pluvia background daemon
    Daemon {
        /// Optional UNIX domain socket path to bind
        #[arg(short, long)]
        socket: Option<PathBuf>,
        /// Run with mock display backend
        #[arg(long)]
        mock: bool,
    },

    /// Launch the Pluvia Studio management GUI
    Studio,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let socket_path = cli.socket.unwrap_or_else(default_socket_path);
    let client = PluviaClient::new(socket_path);

    match cli.command {
        Commands::Ping => {
            let res = client.ping().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "result": res }))?);
            } else {
                println!("Pong: {}", res);
            }
        }
        Commands::Status => {
            let state = client.get_state().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&state)?);
            } else {
                println!("Pluvia Daemon Status:");
                println!("  Status:   {}", state.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"));
                println!("  Uptime:   {}s", state.get("uptime_secs").and_then(|v| v.as_u64()).unwrap_or(0));
                println!("  Backend:  {}", state.get("backend").and_then(|v| v.as_str()).unwrap_or("unknown"));
                let count = state.get("active_skins").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                println!("  Active Skins: {}", count);
            }
        }
        Commands::Load { path } => {
            let res = client.load_skin(&path).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&res)?);
            } else {
                let id = res.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                let path_str = res.get("path").and_then(|v| v.as_str()).unwrap_or("");
                println!("Loaded skin '{}' from {}", id, path_str);
            }
        }
        Commands::Unload { id } => {
            let res = client.unload_skin(&id).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&res)?);
            } else {
                println!("Unloaded skin '{}'", id);
            }
        }
        Commands::List => {
            let skins = client.list_skins().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&skins)?);
            } else {
                let arr = skins.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                println!("Loaded Skins ({}):", arr.len());
                for skin in arr {
                    let id = skin.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                    let path = skin.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                    let measures = skin.get("measures_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    let meters = skin.get("meters_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    let update = skin.get("update_rate_ms").and_then(|v| v.as_u64()).unwrap_or(0);
                    println!("  - [{}] {} (measures: {}, meters: {}, update: {}ms)", id, path, measures, meters, update);
                }
            }
        }
        Commands::Refresh { id } => {
            if let Some(skin_id) = id {
                let res = client.refresh_skin(&skin_id).await?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&res)?);
                } else {
                    println!("Refreshed skin '{}'", skin_id);
                }
            } else {
                let res = client.refresh_all().await?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&res)?);
                } else {
                    let count = res.get("refreshed_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    println!("Refreshed {} skin(s)", count);
                }
            }
        }
        Commands::SetVar { id, key, value } => {
            let res = client.set_variable(&id, &key, &value).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&res)?);
            } else {
                println!("Variable '{}' set to '{}' for skin '{}'", key, value, id);
            }
        }
        Commands::Import { archive_path, dest } => {
            let res = client.import_package(&archive_path, dest.as_ref()).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&res)?);
            } else {
                let files = res.get("files_extracted").and_then(|v| v.as_u64()).unwrap_or(0);
                let bytes = res.get("total_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                let destination = res.get("destination").and_then(|v| v.as_str()).unwrap_or("skins");
                println!("Successfully imported package to {}: {} files extracted ({} bytes)", destination, files, bytes);
            }
        }
        Commands::Pack { source, output } => {
            let report = pluvia_core::extractor::pack_rmskin_package(&source, &output)?;
            if cli.json {
                let res = serde_json::json!({
                    "source": source.to_string_lossy(),
                    "output": output.to_string_lossy(),
                    "files_packaged": report.files_packaged,
                    "total_uncompressed_bytes": report.total_uncompressed_bytes,
                    "package_size": report.package_size,
                });
                println!("{}", serde_json::to_string_pretty(&res)?);
            } else {
                println!(
                    "Successfully packaged '{}' -> '{}': {} files ({} bytes uncompressed, {} bytes package)",
                    source.display(),
                    output.display(),
                    report.files_packaged,
                    report.total_uncompressed_bytes,
                    report.package_size
                );
            }
        }
        Commands::Daemon { socket, mock } => {
            let mut cmd = std::process::Command::new("pluvia-daemon");
            if let Some(sock) = socket {
                cmd.arg("--socket").arg(sock);
            }
            if mock {
                cmd.arg("--mock");
            }
            let status = cmd.status().map_err(|e| {
                format!(
                    "Failed to spawn pluvia-daemon (make sure pluvia-daemon is installed in PATH): {}",
                    e
                )
            })?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::Studio => {
            let status = std::process::Command::new("pluvia-studio")
                .status()
                .map_err(|e| {
                    format!(
                        "Failed to spawn pluvia-studio (make sure pluvia-studio is installed in PATH): {}",
                        e
                    )
                })?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
    }

    Ok(())
}
