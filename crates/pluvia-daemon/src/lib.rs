pub mod display;
pub mod ipc;
pub mod runtime;

pub use display::{
    Anchor, BackendType, DesktopSurface, DisplayError, DisplayLayout, ScreenGeometry, SurfaceBounds,
};
pub use ipc::{default_socket_path, IpcHandle, IpcServer};
pub use runtime::{DaemonState, RuntimeError, SkinInfo, SkinRuntime};
