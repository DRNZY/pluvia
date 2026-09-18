use super::{BackendType, DesktopSurface, DisplayError, SurfaceBounds};
use pluvia_core::render::{AlphaHitMask, Rect};
use std::sync::{Arc, Mutex};

type Display = libc::c_void;
type Visual = libc::c_void;
type Window = libc::c_ulong;
type Colormap = libc::c_ulong;
type Atom = libc::c_ulong;

#[repr(C)]
struct XVisualInfo {
    visual: *mut Visual,
    visualid: libc::c_ulong,
    screen: libc::c_int,
    depth: libc::c_int,
    class: libc::c_int,
    red_mask: libc::c_ulong,
    green_mask: libc::c_ulong,
    blue_mask: libc::c_ulong,
    colormap_size: libc::c_int,
    bits_per_rgb: libc::c_int,
}

#[repr(C)]
struct XSetWindowAttributes {
    background_pixmap: libc::c_ulong,
    background_pixel: libc::c_ulong,
    border_pixmap: libc::c_ulong,
    border_pixel: libc::c_ulong,
    bit_gravity: libc::c_int,
    win_gravity: libc::c_int,
    backing_store: libc::c_int,
    backing_planes: libc::c_ulong,
    backing_pixel: libc::c_ulong,
    save_under: libc::c_int,
    event_mask: libc::c_long,
    do_not_propagate_mask: libc::c_long,
    override_redirect: libc::c_int,
    colormap: Colormap,
    cursor: libc::c_ulong,
}

#[repr(C)]
struct XRectangle {
    x: libc::c_short,
    y: libc::c_short,
    width: libc::c_ushort,
    height: libc::c_ushort,
}

#[link(name = "X11")]
#[link(name = "Xext")]
extern "C" {
    fn XOpenDisplay(name: *const libc::c_char) -> *mut Display;
    fn XCloseDisplay(display: *mut Display) -> libc::c_int;
    fn XDefaultScreen(display: *mut Display) -> libc::c_int;
    fn XRootWindow(display: *mut Display, screen: libc::c_int) -> Window;
    fn XMatchVisualInfo(
        display: *mut Display,
        screen: libc::c_int,
        depth: libc::c_int,
        class: libc::c_int,
        vinfo: *mut XVisualInfo,
    ) -> libc::c_int;
    fn XCreateColormap(
        display: *mut Display,
        w: Window,
        visual: *mut Visual,
        alloc: libc::c_int,
    ) -> Colormap;
    fn XCreateWindow(
        display: *mut Display,
        parent: Window,
        x: libc::c_int,
        y: libc::c_int,
        width: libc::c_uint,
        height: libc::c_uint,
        border_width: libc::c_uint,
        depth: libc::c_int,
        class: libc::c_uint,
        visual: *mut Visual,
        valuemask: libc::c_ulong,
        attributes: *mut XSetWindowAttributes,
    ) -> Window;
    fn XDestroyWindow(display: *mut Display, w: Window) -> libc::c_int;
    fn XMapWindow(display: *mut Display, w: Window) -> libc::c_int;
    fn XUnmapWindow(display: *mut Display, w: Window) -> libc::c_int;
    fn XMoveResizeWindow(
        display: *mut Display,
        w: Window,
        x: libc::c_int,
        y: libc::c_int,
        width: libc::c_uint,
        height: libc::c_uint,
    ) -> libc::c_int;
    fn XInternAtom(
        display: *mut Display,
        atom_name: *const libc::c_char,
        only_if_exists: libc::c_int,
    ) -> Atom;
    fn XChangeProperty(
        display: *mut Display,
        w: Window,
        property: Atom,
        type_: Atom,
        format: libc::c_int,
        mode: libc::c_int,
        data: *const libc::c_uchar,
        nelements: libc::c_int,
    ) -> libc::c_int;
    fn XFlush(display: *mut Display) -> libc::c_int;
    fn XShapeCombineRectangles(
        display: *mut Display,
        dest: Window,
        destKind: libc::c_int,
        xOff: libc::c_int,
        yOff: libc::c_int,
        rectangles: *const XRectangle,
        n_rects: libc::c_int,
        op: libc::c_int,
        ordering: libc::c_int,
    );
}

extern "C" {
    fn cairo_xlib_surface_create(
        dpy: *mut Display,
        drawable: Window,
        visual: *mut Visual,
        width: libc::c_int,
        height: libc::c_int,
    ) -> *mut cairo::ffi::cairo_surface_t;
    fn cairo_xlib_surface_set_size(
        surface: *mut cairo::ffi::cairo_surface_t,
        width: libc::c_int,
        height: libc::c_int,
    );
}

struct NativeX11 {
    display: *mut Display,
    window: Window,
    cairo_surface: *mut cairo::ffi::cairo_surface_t,
    current_w: u32,
    current_h: u32,
}

unsafe impl Send for NativeX11 {}
unsafe impl Sync for NativeX11 {}

impl Drop for NativeX11 {
    fn drop(&mut self) {
        unsafe {
            if !self.cairo_surface.is_null() {
                cairo::ffi::cairo_surface_destroy(self.cairo_surface);
                self.cairo_surface = std::ptr::null_mut();
            }
            if !self.display.is_null() {
                if self.window != 0 {
                    XDestroyWindow(self.display, self.window);
                    self.window = 0;
                }
                XCloseDisplay(self.display);
                self.display = std::ptr::null_mut();
            }
        }
    }
}

/// X11 desktop window surface with EWMH desktop hints and XShape/XFixes click-through mask.
#[derive(Clone)]
pub struct X11Surface {
    id: String,
    bounds: SurfaceBounds,
    window_id: u32,
    window_type_atom: String,
    state_atoms: Vec<String>,
    desktop_atom_value: u32,
    visible: bool,
    destroyed: bool,
    shape_input_rects: Vec<Rect>,
    damage_rects: Vec<Rect>,
    native: Arc<Mutex<Option<NativeX11>>>,
}

impl std::fmt::Debug for X11Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("X11Surface")
            .field("id", &self.id)
            .field("bounds", &self.bounds)
            .field("window_id", &self.window_id)
            .field("visible", &self.visible)
            .field("destroyed", &self.destroyed)
            .finish()
    }
}

impl X11Surface {
    pub fn new(id: impl Into<String>, bounds: SurfaceBounds, window_id: u32) -> Self {
        Self {
            id: id.into(),
            bounds,
            window_id,
            window_type_atom: "_NET_WM_WINDOW_TYPE_DESKTOP".to_string(),
            state_atoms: vec![
                "_NET_WM_STATE_BELOW".to_string(),
                "_NET_WM_STATE_STICKY".to_string(),
            ],
            desktop_atom_value: 0xFFFFFFFF,
            visible: true,
            destroyed: false,
            shape_input_rects: Vec::new(),
            damage_rects: Vec::new(),
            native: Arc::new(Mutex::new(None)),
        }
    }

    pub fn window_id(&self) -> u32 {
        self.window_id
    }

    pub fn window_type_atom(&self) -> &str {
        &self.window_type_atom
    }

    pub fn has_state_atom(&self, atom: &str) -> bool {
        self.state_atoms.iter().any(|a| a == atom)
    }

    pub fn state_atoms(&self) -> &[String] {
        &self.state_atoms
    }

    pub fn desktop_atom_value(&self) -> u32 {
        self.desktop_atom_value
    }

    pub fn shape_input_rects(&self) -> &[Rect] {
        &self.shape_input_rects
    }

    pub fn damage_rects(&self) -> &[Rect] {
        &self.damage_rects
    }

    fn open_native_window(bounds: &SurfaceBounds) -> Option<NativeX11> {
        unsafe {
            let display = XOpenDisplay(std::ptr::null());
            if display.is_null() {
                return None;
            }
            let screen = XDefaultScreen(display);
            let root = XRootWindow(display, screen);

            let mut vinfo = std::mem::zeroed::<XVisualInfo>();
            let status = XMatchVisualInfo(display, screen, 32, 4 /* TrueColor */, &mut vinfo);
            if status == 0 {
                XCloseDisplay(display);
                return None;
            }

            let colormap = XCreateColormap(display, root, vinfo.visual, 0 /* AllocNone */);
            let mut attrs = std::mem::zeroed::<XSetWindowAttributes>();
            attrs.colormap = colormap;
            attrs.background_pixel = 0;
            attrs.border_pixel = 0;
            attrs.override_redirect = 1;

            let w = bounds.width.max(1);
            let h = bounds.height.max(1);
            let win = XCreateWindow(
                display,
                root,
                bounds.x,
                bounds.y,
                w,
                h,
                0,
                32,
                1, /* InputOutput */
                vinfo.visual,
                (1 << 13) | (1 << 3) | (1 << 1) | (1 << 9),
                &mut attrs,
            );

            // EWMH atoms
            let type_atom = XInternAtom(display, b"_NET_WM_WINDOW_TYPE\0".as_ptr() as *const _, 0);
            let dock_atom = XInternAtom(display, b"_NET_WM_WINDOW_TYPE_DOCK\0".as_ptr() as *const _, 0);
            XChangeProperty(display, win, type_atom, 4 /* XA_ATOM */, 32, 0, &dock_atom as *const _ as *const libc::c_uchar, 1);

            let state_atom = XInternAtom(display, b"_NET_WM_STATE\0".as_ptr() as *const _, 0);
            let below_atom = XInternAtom(display, b"_NET_WM_STATE_BELOW\0".as_ptr() as *const _, 0);
            let sticky_atom = XInternAtom(display, b"_NET_WM_STATE_STICKY\0".as_ptr() as *const _, 0);
            let states = [below_atom, sticky_atom];
            XChangeProperty(display, win, state_atom, 4, 32, 0, states.as_ptr() as *const libc::c_uchar, 2);

            let desktop_atom = XInternAtom(display, b"_NET_WM_DESKTOP\0".as_ptr() as *const _, 0);
            let all_desktops: libc::c_ulong = 0xFFFFFFFF;
            XChangeProperty(display, win, desktop_atom, 6 /* XA_CARDINAL */, 32, 0, &all_desktops as *const _ as *const libc::c_uchar, 1);

            XMapWindow(display, win);
            XFlush(display);

            let cairo_surface = cairo_xlib_surface_create(
                display,
                win,
                vinfo.visual,
                w as libc::c_int,
                h as libc::c_int,
            );

            if cairo_surface.is_null() {
                XDestroyWindow(display, win);
                XCloseDisplay(display);
                return None;
            }

            Some(NativeX11 {
                display,
                window: win,
                cairo_surface,
                current_w: w,
                current_h: h,
            })
        }
    }
}

impl DesktopSurface for X11Surface {
    fn id(&self) -> &str {
        &self.id
    }

    fn backend_type(&self) -> BackendType {
        BackendType::X11
    }

    fn bounds(&self) -> SurfaceBounds {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: SurfaceBounds) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.bounds = bounds;
        let guard = self.native.lock().unwrap();
        if let Some(ref n) = *guard {
            unsafe {
                XMoveResizeWindow(
                    n.display,
                    n.window,
                    bounds.x,
                    bounds.y,
                    bounds.width.max(1),
                    bounds.height.max(1),
                );
                cairo_xlib_surface_set_size(
                    n.cairo_surface,
                    bounds.width.max(1) as libc::c_int,
                    bounds.height.max(1) as libc::c_int,
                );
                XFlush(n.display);
            }
        }
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.visible && !self.destroyed
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.visible = visible;
        let guard = self.native.lock().unwrap();
        if let Some(ref n) = *guard {
            unsafe {
                if visible {
                    XMapWindow(n.display, n.window);
                } else {
                    XUnmapWindow(n.display, n.window);
                }
                XFlush(n.display);
            }
        }
        Ok(())
    }

    fn update_surface(
        &mut self,
        surface: &cairo::ImageSurface,
        hit_mask: &AlphaHitMask,
    ) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.shape_input_rects = hit_mask.to_rectangles();
        self.bounds.width = surface.width() as u32;
        self.bounds.height = surface.height() as u32;
        self.damage_rects.push(Rect::new(
            0.0,
            0.0,
            surface.width() as f64,
            surface.height() as f64,
        ));

        // Connect/lazy-open native X11 window if possible
        let mut guard = self.native.lock().unwrap();
        if guard.is_none() && std::env::var("DISPLAY").is_ok() {
            *guard = Self::open_native_window(&self.bounds);
            if let Some(ref n) = *guard {
                self.window_id = n.window as u32;
            }
        }

        if let Some(ref mut n) = *guard {
            unsafe {
                let w = surface.width() as u32;
                let h = surface.height() as u32;
                if n.current_w != w || n.current_h != h {
                    XMoveResizeWindow(
                        n.display,
                        n.window,
                        self.bounds.x,
                        self.bounds.y,
                        w.max(1),
                        h.max(1),
                    );
                    cairo_xlib_surface_set_size(
                        n.cairo_surface,
                        w.max(1) as libc::c_int,
                        h.max(1) as libc::c_int,
                    );
                    n.current_w = w;
                    n.current_h = h;
                }

                // Paint image surface to xlib surface
                let dest = cairo::Surface::from_raw_none(n.cairo_surface);
                if let Ok(cr) = cairo::Context::new(&dest) {
                    cr.set_operator(cairo::Operator::Source);
                    let _ = cr.set_source_surface(surface, 0.0, 0.0);
                    let _ = cr.paint();
                }
                cairo::ffi::cairo_surface_flush(n.cairo_surface);

                // Update 1-bit input shape mask for click-through
                let rects = hit_mask.to_rectangles();
                let xrects: Vec<XRectangle> = rects
                    .iter()
                    .map(|r| XRectangle {
                        x: r.x as libc::c_short,
                        y: r.y as libc::c_short,
                        width: r.width as libc::c_ushort,
                        height: r.height as libc::c_ushort,
                    })
                    .collect();

                if xrects.is_empty() {
                    let dummy = XRectangle {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    };
                    XShapeCombineRectangles(
                        n.display,
                        n.window,
                        2, /* ShapeInput */
                        0,
                        0,
                        &dummy,
                        0,
                        0, /* ShapeSet */
                        0,
                    );
                } else {
                    XShapeCombineRectangles(
                        n.display,
                        n.window,
                        2, /* ShapeInput */
                        0,
                        0,
                        xrects.as_ptr(),
                        xrects.len() as libc::c_int,
                        0, /* ShapeSet */
                        1, /* YXBanded */
                    );
                }

                XFlush(n.display);
            }
        }

        Ok(())
    }

    fn update_hit_mask(&mut self, hit_mask: &AlphaHitMask) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.shape_input_rects = hit_mask.to_rectangles();
        Ok(())
    }

    fn destroy(&mut self) -> Result<(), DisplayError> {
        self.destroyed = true;
        self.shape_input_rects.clear();
        let mut guard = self.native.lock().unwrap();
        *guard = None;
        Ok(())
    }

    fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    fn clear_damage(&mut self) {
        self.damage_rects.clear();
    }
}
