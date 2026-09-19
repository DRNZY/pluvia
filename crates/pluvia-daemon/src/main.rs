use pluvia_daemon::display::BackendType;
use pluvia_daemon::ipc::{default_socket_path, IpcServer};
use pluvia_daemon::runtime::{start_background_ticker, SkinRuntime};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut socket_path: Option<PathBuf> = None;
    let mut backend_override: Option<BackendType> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket" | "-s" => {
                if let Some(val) = args.next() {
                    socket_path = Some(PathBuf::from(val));
                }
            }
            "--mock" => {
                backend_override = Some(BackendType::Mock);
            }
            "--help" | "-h" => {
                println!("Pluvia Daemon - Rainmeter Skin Engine for Linux");
                println!();
                println!("Usage: pluvia-daemon [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -s, --socket <PATH>  UNIX domain socket path to bind");
                println!("      --mock           Use hermetic mock display backend");
                println!("  -h, --help           Print help information");
                return Ok(());
            }
            _ => {}
        }
    }

    let socket_path = socket_path.unwrap_or_else(default_socket_path);
    let backend = backend_override.unwrap_or_else(BackendType::detect);

    println!("Starting Pluvia daemon...");
    println!("  Display Backend: {:?}", backend);
    println!("  Socket Path:     {}", socket_path.display());

    let runtime = Arc::new(RwLock::new(SkinRuntime::new(backend)));

    // Enable X11 multi-threading support (required for spawn_blocking X11 queries)
    pluvia_daemon::display::x11::init_threads();

    // Background ticker loop
    let (shutdown_ticker_tx, shutdown_ticker_rx) = watch::channel(false);
    let _ticker_handle = start_background_ticker(runtime.clone(), shutdown_ticker_rx);

    // IPC server
    let server = IpcServer::new(&socket_path, runtime);
    let handle = server.spawn()?;

    println!("Pluvia daemon is ready and listening for IPC commands.");
    println!("Press Ctrl+C to terminate.");

    tokio::signal::ctrl_c().await?;
    println!("\nShutdown signal received. Stopping daemon...");

    let _ = shutdown_ticker_tx.send(true);
    handle.stop();

    println!("Pluvia daemon shut down cleanly.");
    Ok(())
}
