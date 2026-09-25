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

#[repr(C)]
pub struct XEvent {
    pad: [libc::c_long; 24],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XButtonEvent {
    type_: libc::c_int,
    serial: libc::c_ulong,
    send_event: libc::c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: libc::c_ulong,
    x: libc::c_int,
    y: libc::c_int,
    x_root: libc::c_int,
    y_root: libc::c_int,
    state: libc::c_uint,
    button: libc::c_uint,
    same_screen: libc::c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XMotionEvent {
    type_: libc::c_int,
    serial: libc::c_ulong,
    send_event: libc::c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: libc::c_ulong,
    x: libc::c_int,
    y: libc::c_int,
    x_root: libc::c_int,
    y_root: libc::c_int,
    state: libc::c_uint,
    is_hint: libc::c_char,
    same_screen: libc::c_int,
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
    fn XMoveWindow(display: *mut Display, w: Window, x: libc::c_int, y: libc::c_int) -> libc::c_int;
    fn XLowerWindow(display: *mut Display, w: Window) -> libc::c_int;
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
    fn XGetWindowProperty(
        display: *mut Display,
        w: Window,
        property: Atom,
        long_offset: libc::c_long,
        long_length: libc::c_long,
        delete: libc::c_int,
        req_type: Atom,
        actual_type_return: *mut Atom,
        actual_format_return: *mut libc::c_int,
        nitems_return: *mut libc::c_ulong,
        bytes_after_return: *mut libc::c_ulong,
        prop_return: *mut *mut libc::c_uchar,
    ) -> libc::c_int;
    fn XFree(data: *mut libc::c_void) -> libc::c_int;
    fn XPending(display: *mut Display) -> libc::c_int;
    fn XNextEvent(display: *mut Display, event_return: *mut XEvent) -> libc::c_int;
    fn XFlush(display: *mut Display) -> libc::c_int;
    fn XSetErrorHandler(
        handler: Option<unsafe extern "C" fn(*mut Display, *mut libc::c_void) -> libc::c_int>,
    ) -> libc::c_int;
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
    fn XQueryTree(
        display: *mut Display,
        w: Window,
        root_return: *mut Window,
        parent_return: *mut Window,
        children_return: *mut *mut Window,
        nchildren_return: *mut libc::c_uint,
    ) -> libc::c_int;
    fn XGetGeometry(
        display: *mut Display,
        drawable: Window,
        root_return: *mut Window,
        x_return: *mut libc::c_int,
        y_return: *mut libc::c_int,
        width_return: *mut libc::c_uint,
        height_return: *mut libc::c_uint,
        border_width_return: *mut libc::c_uint,
        depth_return: *mut libc::c_uint,
    ) -> libc::c_int;
    fn XTranslateCoordinates(
        display: *mut Display,
        src_w: Window,
        dest_w: Window,
        src_x: libc::c_int,
        src_y: libc::c_int,
        dest_x_return: *mut libc::c_int,
        dest_y_return: *mut libc::c_int,
        child_return: *mut Window,
    ) -> libc::c_int;
    fn XInitThreads() -> libc::c_int;
}

unsafe extern "C" fn x11_error_handler(_dpy: *mut Display, _err: *mut libc::c_void) -> libc::c_int {
    0
}

/// Must be called once at process startup before any X11 functions are called from multiple threads.
pub fn init_threads() {
    unsafe {
        XInitThreads();
    }
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
    drag_start: Option<(i32, i32, i32, i32)>,
}

impl NativeX11 {
    fn poll_events(&mut self, bounds: &mut SurfaceBounds) {
        unsafe {
            let mut ev = std::mem::zeroed::<XEvent>();
            while XPending(self.display) > 0 {
                XNextEvent(self.display, &mut ev);
                let ev_type = *(ev.pad.as_ptr() as *const libc::c_int);
                match ev_type {
                    4 /* ButtonPress */ => {
                        let btn = *(ev.pad.as_ptr() as *const XButtonEvent);
                        if btn.button == 1 {
                            self.drag_start = Some((btn.x_root, btn.y_root, bounds.x, bounds.y));
                        }
                    }
                    5 /* ButtonRelease */ => {
                        let btn = *(ev.pad.as_ptr() as *const XButtonEvent);
                        if btn.button == 1 {
                            self.drag_start = None;
                        }
                    }
                    6 /* MotionNotify */ => {
                        if let Some((start_rx, start_ry, start_wx, start_wy)) = self.drag_start {
                            let motion = *(ev.pad.as_ptr() as *const XMotionEvent);
                            let dx = motion.x_root - start_rx;
                            let dy = motion.y_root - start_ry;
                            let new_x = start_wx + dx;
                            let new_y = start_wy + dy;
                            bounds.x = new_x;
                            bounds.y = new_y;
                            XMoveWindow(self.display, self.window, new_x, new_y);
                            XFlush(self.display);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
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
    opacity: f64,
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
            .field("opacity", &self.opacity)
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
            opacity: 1.0,
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

    fn open_native_window(bounds: &SurfaceBounds, opacity: f64) -> Option<NativeX11> {
        unsafe {
            XSetErrorHandler(Some(x11_error_handler));
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
            attrs.override_redirect = 0;
            attrs.event_mask = (1 << 2) /* ButtonPress */
                | (1 << 3) /* ButtonRelease */
                | (1 << 6) /* PointerMotion */
                | (1 << 8) /* StructureNotify */
                | (1 << 13) /* Button1Motion */;

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
                (1 << 13) | (1 << 11) | (1 << 3) | (1 << 1) | (1 << 9),
                &mut attrs,
            );

            // EWMH window type: _NET_WM_WINDOW_TYPE_DESKTOP
            let type_atom = XInternAtom(display, b"_NET_WM_WINDOW_TYPE\0".as_ptr() as *const _, 0);
            let desktop_type_atom = XInternAtom(display, b"_NET_WM_WINDOW_TYPE_DESKTOP\0".as_ptr() as *const _, 0);
            XChangeProperty(display, win, type_atom, 4 /* XA_ATOM */, 32, 0, &desktop_type_atom as *const _ as *const libc::c_uchar, 1);

            // EWMH window states: BELOW, STICKY, SKIP_TASKBAR, SKIP_PAGER
            let state_atom = XInternAtom(display, b"_NET_WM_STATE\0".as_ptr() as *const _, 0);
            let below_atom = XInternAtom(display, b"_NET_WM_STATE_BELOW\0".as_ptr() as *const _, 0);
            let sticky_atom = XInternAtom(display, b"_NET_WM_STATE_STICKY\0".as_ptr() as *const _, 0);
            let skip_tb_atom = XInternAtom(display, b"_NET_WM_STATE_SKIP_TASKBAR\0".as_ptr() as *const _, 0);
            let skip_pager_atom = XInternAtom(display, b"_NET_WM_STATE_SKIP_PAGER\0".as_ptr() as *const _, 0);
            let states = [below_atom, sticky_atom, skip_tb_atom, skip_pager_atom];
            XChangeProperty(display, win, state_atom, 4, 32, 0, states.as_ptr() as *const libc::c_uchar, 4);

            // EWMH desktop: all desktops / workspaces (0xFFFFFFFF)
            let desktop_atom = XInternAtom(display, b"_NET_WM_DESKTOP\0".as_ptr() as *const _, 0);
            let all_desktops: libc::c_ulong = 0xFFFFFFFF;
            XChangeProperty(display, win, desktop_atom, 6 /* XA_CARDINAL */, 32, 0, &all_desktops as *const _ as *const libc::c_uchar, 1);

            // Motif hints: remove all window borders and titlebar
            let motif_atom = XInternAtom(display, b"_MOTIF_WM_HINTS\0".as_ptr() as *const _, 0);
            let motif_hints: [libc::c_ulong; 5] = [2 /* MWM_HINTS_DECORATIONS */, 0, 0 /* no decorations */, 0, 0];
            XChangeProperty(display, win, motif_atom, motif_atom, 32, 0, motif_hints.as_ptr() as *const libc::c_uchar, 5);

            // WM_NORMAL_HINTS size & position hints
            let normal_hints_atom = XInternAtom(display, b"WM_NORMAL_HINTS\0".as_ptr() as *const _, 0);
            let size_hints_atom = XInternAtom(display, b"WM_SIZE_HINTS\0".as_ptr() as *const _, 0);
            let size_hints: [libc::c_long; 18] = [
                (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3), /* USPosition | USSize | PPosition | PSize */
                bounds.x as libc::c_long, bounds.y as libc::c_long,
                w as libc::c_long, h as libc::c_long,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ];
            XChangeProperty(display, win, normal_hints_atom, size_hints_atom, 32, 0, size_hints.as_ptr() as *const libc::c_uchar, 18);

            let utf8_string = XInternAtom(display, b"UTF8_STRING\0".as_ptr() as *const _, 0);
            let name_atom = XInternAtom(display, b"_NET_WM_NAME\0".as_ptr() as *const _, 0);
            let title = b"Pluvia\0";
            XChangeProperty(display, win, name_atom, utf8_string, 8, 0, title.as_ptr(), 6);
            let wm_name = XInternAtom(display, b"WM_NAME\0".as_ptr() as *const _, 0);
            XChangeProperty(display, win, wm_name, 31 /* XA_STRING */, 8, 0, title.as_ptr(), 6);

            // Apply opacity
            let opacity_atom = XInternAtom(display, b"_NET_WM_WINDOW_OPACITY\0".as_ptr() as *const _, 0);
            let cardinal_atom = XInternAtom(display, b"CARDINAL\0".as_ptr() as *const _, 0);
            let alpha = opacity.clamp(0.0, 1.0);
            let val: u32 = (alpha * 4294967295.0).round() as u32;
            XChangeProperty(display, win, opacity_atom, cardinal_atom, 32, 0, &val as *const _ as *const libc::c_uchar, 1);

            XMapWindow(display, win);
            // Lower window to desktop bottom layer so normal windows naturally sit above it
            XLowerWindow(display, win);
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
                drag_start: None,
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
            *guard = Self::open_native_window(&self.bounds, self.opacity);
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

                // Update 1-bit input shape mask for click-through only when shape changes
                let rects = hit_mask.to_rectangles();
                if rects != self.shape_input_rects {
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
                            0, /* Unsorted */
                        );
                    }
                    self.shape_input_rects = rects;
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

    fn set_opacity(&mut self, opacity: f64) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.opacity = opacity;
        let guard = self.native.lock().unwrap();
        if let Some(ref n) = *guard {
            unsafe {
                let opacity_atom = XInternAtom(n.display, b"_NET_WM_WINDOW_OPACITY\0".as_ptr() as *const _, 0);
                let cardinal_atom = XInternAtom(n.display, b"CARDINAL\0".as_ptr() as *const _, 0);
                let alpha = opacity.clamp(0.0, 1.0);
                let val: u32 = (alpha * 4294967295.0).round() as u32;
                XChangeProperty(
                    n.display,
                    n.window,
                    opacity_atom,
                    cardinal_atom,
                    32,
                    0,
                    &val as *const _ as *const libc::c_uchar,
                    1,
                );
                XFlush(n.display);
            }
        }
        Ok(())
    }

    fn poll_events(&mut self) {
        if self.destroyed {
            return;
        }
        let mut guard = self.native.lock().unwrap();
        if let Some(ref mut n) = *guard {
            n.poll_events(&mut self.bounds);
        }
    }
}

/// Queries the X11 server to determine if the currently active window is in fullscreen mode.
pub fn is_fullscreen_window_active() -> bool {
    if std::env::var("DISPLAY").is_err() {
        return false;
    }
    unsafe {
        XSetErrorHandler(Some(x11_error_handler));
        let display = XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return false;
        }
        let screen = XDefaultScreen(display);
        let root = XRootWindow(display, screen);

        let net_active_window = XInternAtom(display, b"_NET_ACTIVE_WINDOW\0".as_ptr() as *const _, 0);
        let net_wm_state = XInternAtom(display, b"_NET_WM_STATE\0".as_ptr() as *const _, 0);
        let net_wm_state_fullscreen = XInternAtom(display, b"_NET_WM_STATE_FULLSCREEN\0".as_ptr() as *const _, 0);

        let mut actual_type: Atom = 0;
        let mut actual_format: libc::c_int = 0;
        let mut nitems: libc::c_ulong = 0;
        let mut bytes_after: libc::c_ulong = 0;
        let mut prop: *mut libc::c_uchar = std::ptr::null_mut();

        let status = XGetWindowProperty(
            display,
            root,
            net_active_window,
            0,
            1,
            0,
            33, /* XA_WINDOW */
            &mut actual_type,
            &mut actual_format,
            &mut nitems,
            &mut bytes_after,
            &mut prop,
        );

        let mut is_fullscreen = false;
        if status == 0 && !prop.is_null() && nitems > 0 {
            let active_win = *(prop as *const Window);
            XFree(prop as *mut libc::c_void);

            if active_win != 0 {
                let mut state_prop: *mut libc::c_uchar = std::ptr::null_mut();
                let status_state = XGetWindowProperty(
                    display,
                    active_win,
                    net_wm_state,
                    0,
                    1024,
                    0,
                    4, /* XA_ATOM */
                    &mut actual_type,
                    &mut actual_format,
                    &mut nitems,
                    &mut bytes_after,
                    &mut state_prop,
                );

                if status_state == 0 && !state_prop.is_null() {
                    let atoms = std::slice::from_raw_parts(state_prop as *const Atom, nitems as usize);
                    if atoms.contains(&net_wm_state_fullscreen) {
                        is_fullscreen = true;
                    }
                    XFree(state_prop as *mut libc::c_void);
                }
            }
        }

        XCloseDisplay(display);
        is_fullscreen
    }
}

/// Returns bounding rects (in root/screen coords) of all mapped normal application windows.
/// Used to detect when desktop widgets are covered by app windows.
pub fn get_visible_normal_window_rects() -> Vec<SurfaceBounds> {
    if std::env::var("DISPLAY").is_err() {
        return Vec::new();
    }
    let mut rects = Vec::new();
    unsafe {
        XSetErrorHandler(Some(x11_error_handler));
        let display = XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return rects;
        }
        let screen = XDefaultScreen(display);
        let root = XRootWindow(display, screen);

        let net_wm_state = XInternAtom(display, b"_NET_WM_STATE\0".as_ptr() as *const _, 0);
        let net_wm_state_hidden = XInternAtom(display, b"_NET_WM_STATE_HIDDEN\0".as_ptr() as *const _, 0);
        let net_wm_window_type = XInternAtom(display, b"_NET_WM_WINDOW_TYPE\0".as_ptr() as *const _, 0);
        let net_wm_window_type_normal = XInternAtom(display, b"_NET_WM_WINDOW_TYPE_NORMAL\0".as_ptr() as *const _, 0);
        let net_wm_window_type_dialog = XInternAtom(display, b"_NET_WM_WINDOW_TYPE_DIALOG\0".as_ptr() as *const _, 0);

        let mut root_ret: Window = 0;
        let mut parent_ret: Window = 0;
        let mut children: *mut Window = std::ptr::null_mut();
        let mut nchildren: libc::c_uint = 0;

        if XQueryTree(display, root, &mut root_ret, &mut parent_ret, &mut children, &mut nchildren) != 0
            && !children.is_null()
        {
            let window_list = std::slice::from_raw_parts(children, nchildren as usize);
            for &win in window_list {
                // Check window type — only include normal/dialog windows
                let mut actual_type: Atom = 0;
                let mut actual_format: libc::c_int = 0;
                let mut nitems: libc::c_ulong = 0;
                let mut bytes_after: libc::c_ulong = 0;
                let mut type_prop: *mut libc::c_uchar = std::ptr::null_mut();

                let type_ok = XGetWindowProperty(
                    display, win, net_wm_window_type, 0, 4, 0, 4 /* XA_ATOM */,
                    &mut actual_type, &mut actual_format, &mut nitems,
                    &mut bytes_after, &mut type_prop,
                ) == 0 && !type_prop.is_null() && nitems > 0;

                let is_normal_type = if type_ok {
                    let atoms = std::slice::from_raw_parts(type_prop as *const Atom, nitems as usize);
                    let result = atoms.contains(&net_wm_window_type_normal)
                        || atoms.contains(&net_wm_window_type_dialog);
                    XFree(type_prop as *mut libc::c_void);
                    result
                } else {
                    // No _NET_WM_WINDOW_TYPE set — treat as normal if it has no type (older apps)
                    if !type_prop.is_null() { XFree(type_prop as *mut libc::c_void); }
                    false
                };

                if !is_normal_type {
                    continue;
                }

                // Check if window is hidden/minimized
                let mut state_prop: *mut libc::c_uchar = std::ptr::null_mut();
                let state_ok = XGetWindowProperty(
                    display, win, net_wm_state, 0, 64, 0, 4 /* XA_ATOM */,
                    &mut actual_type, &mut actual_format, &mut nitems,
                    &mut bytes_after, &mut state_prop,
                ) == 0 && !state_prop.is_null();

                let is_hidden = if state_ok {
                    let atoms = std::slice::from_raw_parts(state_prop as *const Atom, nitems as usize);
                    let hidden = atoms.contains(&net_wm_state_hidden);
                    XFree(state_prop as *mut libc::c_void);
                    hidden
                } else {
                    if !state_prop.is_null() { XFree(state_prop as *mut libc::c_void); }
                    false
                };

                if is_hidden {
                    continue;
                }

                // Get geometry — coordinates are relative to the parent (usually root)
                let mut geom_root: Window = 0;
                let mut gx: libc::c_int = 0;
                let mut gy: libc::c_int = 0;
                let mut gw: libc::c_uint = 0;
                let mut gh: libc::c_uint = 0;
                let mut bw: libc::c_uint = 0;
                let mut depth: libc::c_uint = 0;

                if XGetGeometry(display, win, &mut geom_root, &mut gx, &mut gy, &mut gw, &mut gh, &mut bw, &mut depth) == 0
                    || gw == 0 || gh == 0
                {
                    continue;
                }

                // Translate to root/screen coords
                let mut rx: libc::c_int = 0;
                let mut ry: libc::c_int = 0;
                let mut child: Window = 0;
                XTranslateCoordinates(display, win, root, 0, 0, &mut rx, &mut ry, &mut child);

                rects.push(SurfaceBounds::new(rx, ry, gw, gh));
            }
            XFree(children as *mut libc::c_void);
        }

        XCloseDisplay(display);
    }
    rects
}
