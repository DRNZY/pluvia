import math
import subprocess
import cairo
import numpy as np

WIDTH = 1920
HEIGHT = 1080
FPS = 60
DURATION_SECS = 15.0
TOTAL_FRAMES = int(FPS * DURATION_SECS)

def cubic_bezier_apple(t: float) -> float:
    """Approximation of cubic-bezier(0.16, 1, 0.3, 1) - Apple standard ease out."""
    t = max(0.0, min(1.0, t))
    return 1.0 - math.pow(1.0 - t, 3.5)

def smoothstep(edge0: float, edge1: float, x: float) -> float:
    t = max(0.0, min(1.0, (x - edge0) / (edge1 - edge0)))
    return t * t * (3.0 - 2.0 * t)

def draw_rounded_rect(ctx, x, y, w, h, r):
    ctx.new_sub_path()
    ctx.arc(x + w - r, y + r, r, -math.pi / 2, 0)
    ctx.arc(x + w - r, y + h - r, r, 0, math.pi / 2)
    ctx.arc(x + r, y + h - r, r, math.pi / 2, math.pi)
    ctx.arc(x + r, y + r, r, math.pi, 3 * math.pi / 2)
    ctx.close_path()

def draw_studio_background(ctx, t_norm: float):
    # Pure deep obsidian canvas with subtle studio softbox rim
    ctx.save()
    bg = cairo.RadialGradient(WIDTH / 2, -150, 50, WIDTH / 2, 400, 1400)
    bg.add_color_stop_rgba(0.0, 0.10, 0.11, 0.15, 1.0)
    bg.add_color_stop_rgba(0.4, 0.04, 0.04, 0.06, 1.0)
    bg.add_color_stop_rgba(1.0, 0.02, 0.02, 0.03, 1.0)
    ctx.set_source(bg)
    ctx.paint()
    ctx.restore()

def render_scene_1(ctx, frame: int, t_sec: float):
    """Scene 1: The Void & Reveal (0.0s - 3.8s)"""
    # Progressive reveal of the title with Apple deceleration
    t_in = min(1.0, t_sec / 2.2)
    ease = cubic_bezier_apple(t_in)
    
    # Fade out at the end of scene
    alpha_out = 1.0
    if t_sec > 3.0:
        alpha_out = max(0.0, 1.0 - (t_sec - 3.0) / 0.8)
    
    alpha = ease * alpha_out
    y_offset = (1.0 - ease) * 50.0

    ctx.save()
    
    # Subtle studio spotlight behind title
    spot = cairo.RadialGradient(WIDTH / 2, HEIGHT / 2 - 30 + y_offset, 10, WIDTH / 2, HEIGHT / 2 - 30, 450)
    spot.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.09 * alpha)
    spot.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0)
    ctx.set_source(spot)
    ctx.paint()

    # Brand Title: "Pluvia"
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(118.0)
    
    ext = ctx.text_extents("Pluvia")
    title_x = (WIDTH - ext.width) / 2 - ext.x_bearing
    title_y = HEIGHT / 2 - 40 + y_offset

    # Specular white text
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha)
    ctx.move_to(title_x, title_y)
    ctx.show_text("Pluvia")

    # Subtitle: "The Native Rainmeter Engine for Linux"
    t_sub = min(1.0, max(0.0, (t_sec - 0.6) / 1.8))
    sub_ease = cubic_bezier_apple(t_sub)
    sub_alpha = sub_ease * alpha_out
    sub_y_offset = (1.0 - sub_ease) * 30.0

    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(32.0)
    sub_text = "The Native Rainmeter Engine for Linux."
    sub_ext = ctx.text_extents(sub_text)
    ctx.set_source_rgba(0.53, 0.53, 0.55, sub_alpha)  # Apple Silver #86868B
    ctx.move_to((WIDTH - sub_ext.width) / 2 - sub_ext.x_bearing, HEIGHT / 2 + 55 + sub_y_offset)
    ctx.show_text(sub_text)

    # Feature pills
    t_pills = min(1.0, max(0.0, (t_sec - 1.2) / 1.5))
    pill_ease = cubic_bezier_apple(t_pills)
    pill_alpha = pill_ease * alpha_out

    if pill_alpha > 0.01:
        pills = ["Rust 2021", "Cairo Vector Graphics", "0.4ms Parser", "Wayland & X11"]
        pill_w = 190
        total_pills_w = len(pills) * pill_w + (len(pills) - 1) * 20
        start_px = (WIDTH - total_pills_w) / 2
        py = HEIGHT / 2 + 130

        ctx.set_font_size(15.0)
        for i, p_text in enumerate(pills):
            px = start_px + i * (pill_w + 20)
            
            # Pill card background
            draw_rounded_rect(ctx, px, py, pill_w, 40, 20)
            ctx.set_source_rgba(0.12, 0.12, 0.16, 0.7 * pill_alpha)
            ctx.fill_preserve()
            ctx.set_source_rgba(0.3, 0.3, 0.38, 0.5 * pill_alpha)
            ctx.set_line_width(1.0)
            ctx.stroke()

            # Pill text
            pext = ctx.text_extents(p_text)
            ctx.set_source_rgba(0.85, 0.85, 0.88, pill_alpha)
            ctx.move_to(px + (pill_w - pext.width) / 2 - pext.x_bearing, py + 25)
            ctx.show_text(p_text)

    ctx.restore()

def render_scene_2(ctx, frame: int, t_sec: float):
    """Scene 2: Precision Engineering & Core Architecture (3.5s - 7.5s)"""
    local_t = t_sec - 3.5
    t_in = min(1.0, local_t / 1.2)
    ease = cubic_bezier_apple(t_in)
    
    alpha_out = 1.0
    if local_t > 3.2:
        alpha_out = max(0.0, 1.0 - (local_t - 3.2) / 0.8)
    
    alpha = ease * alpha_out
    slide_y = (1.0 - ease) * 40.0

    ctx.save()

    # Section Title
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(44.0)
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha)
    title = "Built for Uncompromising Performance"
    ext = ctx.text_extents(title)
    ctx.move_to((WIDTH - ext.width) / 2 - ext.x_bearing, 140 + slide_y)
    ctx.show_text(title)

    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(22.0)
    ctx.set_source_rgba(0.53, 0.53, 0.55, alpha)
    sub = "Zero compromise between classic Rainmeter compatibility and Linux desktop performance."
    sext = ctx.text_extents(sub)
    ctx.move_to((WIDTH - sext.width) / 2 - sext.x_bearing, 185 + slide_y)
    ctx.show_text(sub)

    # 3 Architecture Glass Cards
    cards = [
        {
            "metric": "< 0.5 ms",
            "label": "Parser & VFS",
            "desc": "Case-insensitive INI parser with recursive style inheritance and cyclic include graph defense.",
            "accent": (0.04, 0.52, 1.0) # Electric Sapphire / Blue
        },
        {
            "metric": "60 FPS",
            "label": "Cairo Vector Engine",
            "desc": "Sub-pixel font antialiasing, hardware-accelerated shapes, and procedural rotator meters.",
            "accent": (0.19, 0.82, 0.35) # Apple Green
        },
        {
            "metric": "100%",
            "label": "Security Sandbox",
            "desc": "Anti-Zip-Slip canonical path validation and symlink traversal immunity during skin install.",
            "accent": (1.0, 0.62, 0.04) # Apple Amber
        }
    ]

    card_w = 460
    card_h = 360
    total_w = len(cards) * card_w + (len(cards) - 1) * 35
    start_x = (WIDTH - total_w) / 2
    card_y = 260 + slide_y

    for i, c in enumerate(cards):
        cx = start_x + i * (card_w + 35)
        
        # Soft shadow
        draw_rounded_rect(ctx, cx, card_y + 15, card_w, card_h, 24)
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.4 * alpha)
        ctx.fill()

        # Glass Card Body
        draw_rounded_rect(ctx, cx, card_y, card_w, card_h, 24)
        card_grad = cairo.LinearGradient(cx, card_y, cx, card_y + card_h)
        card_grad.add_color_stop_rgba(0.0, 0.10, 0.11, 0.14, 0.9 * alpha)
        card_grad.add_color_stop_rgba(1.0, 0.06, 0.06, 0.08, 0.9 * alpha)
        ctx.set_source(card_grad)
        ctx.fill_preserve()

        # 1px Chamfer Border
        ctx.set_source_rgba(0.24, 0.24, 0.30, 0.6 * alpha)
        ctx.set_line_width(1.2)
        ctx.stroke()

        # Accent Glow Line on Top
        ctx.new_sub_path()
        ctx.arc(cx + card_w - 24, card_y + 24, 24, -math.pi / 2, -math.pi / 3)
        ctx.new_sub_path()
        ctx.arc(cx + 24, card_y + 24, 24, -2 * math.pi / 3, -math.pi / 2)
        ctx.move_to(cx + 24, card_y)
        ctx.line_to(cx + card_w - 24, card_y)
        ctx.set_source_rgba(c["accent"][0], c["accent"][1], c["accent"][2], 0.8 * alpha)
        ctx.set_line_width(2.5)
        ctx.stroke()

        # Metric Number
        ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
        ctx.set_font_size(52.0)
        ctx.set_source_rgba(1.0, 1.0, 1.0, alpha)
        ctx.move_to(cx + 40, card_y + 90)
        ctx.show_text(c["metric"])

        # Metric Label
        ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
        ctx.set_font_size(24.0)
        ctx.set_source_rgba(c["accent"][0], c["accent"][1], c["accent"][2], 0.95 * alpha)
        ctx.move_to(cx + 40, card_y + 140)
        ctx.show_text(c["label"])

        # Description
        ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
        ctx.set_font_size(18.0)
        ctx.set_source_rgba(0.70, 0.70, 0.75, 0.85 * alpha)
        
        # Simple word wrap
        words = c["desc"].split()
        line = ""
        ly = card_y + 195
        for w in words:
            test_line = line + (" " if line else "") + w
            if ctx.text_extents(test_line).width < card_w - 80:
                line = test_line
            else:
                ctx.move_to(cx + 40, ly)
                ctx.show_text(line)
                ly += 28
                line = w
        if line:
            ctx.move_to(cx + 40, ly)
            ctx.show_text(line)

    # Monospace Rust benchmark snippet at the bottom
    bench_y = 680 + slide_y
    draw_rounded_rect(ctx, start_x, bench_y, total_w, 180, 18)
    ctx.set_source_rgba(0.04, 0.04, 0.06, 0.95 * alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(0.18, 0.18, 0.22, 0.8 * alpha)
    ctx.set_line_width(1.0)
    ctx.stroke()

    # Code lines
    ctx.select_font_face("DejaVu Sans Mono", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(15.0)
    code_lines = [
        ("// Native telemetry tick in pluvia-daemon", (0.45, 0.45, 0.50)),
        ("let report = pluvia_core::extractor::pack_rmskin_package(source, dest)?;", (0.9, 0.9, 0.9)),
        ("let measures = skin_runtime.tick_measures(&sysfs_sensors); // 0.12ms", (0.35, 0.85, 0.55)),
        ("surface_compositor.render_damage_rects(&skin_state, &cairo_ctx); // 60 FPS Wayland", (0.4, 0.7, 1.0)),
    ]
    for i, (cline, col) in enumerate(code_lines):
        ctx.set_source_rgba(col[0], col[1], col[2], alpha)
        ctx.move_to(start_x + 35, bench_y + 40 + i * 32)
        ctx.show_text(cline)

    ctx.restore()

def render_scene_3(ctx, frame: int, t_sec: float):
    """Scene 3: Live Desktop Widgets in Motion (7.5s - 12.0s)"""
    local_t = t_sec - 7.5
    t_in = min(1.0, local_t / 1.2)
    ease = cubic_bezier_apple(t_in)
    
    alpha_out = 1.0
    if local_t > 3.8:
        alpha_out = max(0.0, 1.0 - (local_t - 3.8) / 0.7)
    
    alpha = ease * alpha_out
    slide_y = (1.0 - ease) * 35.0

    ctx.save()

    # Headline
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(44.0)
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha)
    title = "Authentic Desktop Widgets. Infinite Flexibility."
    ext = ctx.text_extents(title)
    ctx.move_to((WIDTH - ext.width) / 2 - ext.x_bearing, 120 + slide_y)
    ctx.show_text(title)

    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(20.0)
    ctx.set_source_rgba(0.53, 0.53, 0.55, alpha)
    sub = "Runs any classic or modern Rainmeter skin directly on Wayland (wlroots/Hyprland/Sway/GNOME) and X11."
    sext = ctx.text_extents(sub)
    ctx.move_to((WIDTH - sext.width) / 2 - sext.x_bearing, 160 + slide_y)
    ctx.show_text(sub)

    # 3 Real Live Desktop Skins Side-by-Side
    # 1. Mond Clock Skin
    # 2. Monterey System Telemetry
    # 3. Flint Spectrum Visualizer

    # 1. Mond Clock (Left)
    c1_x = 180
    c1_y = 220 + slide_y
    c1_w = 460
    c1_h = 580
    draw_rounded_rect(ctx, c1_x, c1_y, c1_w, c1_h, 28)
    ctx.set_source_rgba(0.08, 0.08, 0.10, 0.85 * alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(0.22, 0.22, 0.28, 0.6 * alpha)
    ctx.set_line_width(1.2)
    ctx.stroke()

    # Mond skin label
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(14.0)
    ctx.set_source_rgba(0.5, 0.5, 0.55, alpha)
    ctx.move_to(c1_x + 30, c1_y + 40)
    ctx.show_text("SKIN: MOND / CLOCK")

    # Time Display
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(88.0)
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha)
    time_str = "18:14"
    text_ext = ctx.text_extents(time_str)
    ctx.move_to(c1_x + (c1_w - text_ext.width) / 2 - text_ext.x_bearing, c1_y + 180)
    ctx.show_text(time_str)

    # Date
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(24.0)
    ctx.set_source_rgba(0.8, 0.8, 0.85, alpha)
    date_str = "Wednesday, September 23"
    d_ext = ctx.text_extents(date_str)
    ctx.move_to(c1_x + (c1_w - d_ext.width) / 2 - d_ext.x_bearing, c1_y + 240)
    ctx.show_text(date_str)

    # Dynamic weather / quote note
    draw_rounded_rect(ctx, c1_x + 40, c1_y + 300, c1_w - 80, 180, 16)
    ctx.set_source_rgba(0.12, 0.12, 0.16, 0.7 * alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(0.2, 0.2, 0.25, 0.4 * alpha)
    ctx.set_line_width(1.0)
    ctx.stroke()

    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(16.0)
    ctx.set_source_rgba(0.6, 0.6, 0.65, alpha)
    ctx.move_to(c1_x + 60, c1_y + 350)
    ctx.show_text("Live Variable Injection:")
    ctx.set_source_rgba(0.35, 0.75, 1.0, alpha)
    ctx.move_to(c1_x + 60, c1_y + 390)
    ctx.show_text("pluvia-cli set-var Mond Key Value")
    ctx.set_source_rgba(0.8, 0.8, 0.85, alpha)
    ctx.move_to(c1_x + 60, c1_y + 440)
    ctx.show_text("Zero-flicker sub-pixel redraw")

    # 2. Monterey Telemetry HUD (Center)
    c2_x = 730
    c2_y = 220 + slide_y
    c2_w = 460
    c2_h = 580
    draw_rounded_rect(ctx, c2_x, c2_y, c2_w, c2_h, 28)
    ctx.set_source_rgba(0.08, 0.08, 0.10, 0.85 * alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(0.22, 0.22, 0.28, 0.6 * alpha)
    ctx.set_line_width(1.2)
    ctx.stroke()

    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(14.0)
    ctx.set_source_rgba(0.5, 0.5, 0.55, alpha)
    ctx.move_to(c2_x + 30, c2_y + 40)
    ctx.show_text("SKIN: MONTEREY / SYSTEM")

    # Animated CPU / RAM / GPU Ring Gauges
    gauges = [
        {"name": "CPU", "val": 0.32 + 0.12 * math.sin(local_t * 3.0), "col": (0.04, 0.52, 1.0)},
        {"name": "RAM", "val": 0.48 + 0.04 * math.cos(local_t * 2.0), "col": (0.58, 0.34, 0.95)},
        {"name": "GPU", "val": 0.24 + 0.18 * math.sin(local_t * 4.0), "col": (0.19, 0.82, 0.35)},
    ]
    for i, g in enumerate(gauges):
        gx = c2_x + 80 + i * 150
        gy = c2_y + 190
        gr = 48
        
        # Background track
        ctx.set_line_width(8.0)
        ctx.set_source_rgba(0.18, 0.18, 0.22, 0.6 * alpha)
        ctx.arc(gx, gy, gr, 0, 2 * math.pi)
        ctx.stroke()

        # Active arc
        ctx.set_source_rgba(g["col"][0], g["col"][1], g["col"][2], alpha)
        ctx.arc(gx, gy, gr, -math.pi / 2, -math.pi / 2 + 2 * math.pi * g["val"])
        ctx.stroke()

        # Center percentage
        pct_text = f"{int(g['val'] * 100)}%"
        ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
        ctx.set_font_size(18.0)
        ctx.set_source_rgba(1.0, 1.0, 1.0, alpha)
        pe = ctx.text_extents(pct_text)
        ctx.move_to(gx - pe.width / 2 - pe.x_bearing, gy + 7)
        ctx.show_text(pct_text)

        # Label below
        ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
        ctx.set_font_size(14.0)
        ctx.set_source_rgba(0.6, 0.6, 0.65, alpha)
        le = ctx.text_extents(g["name"])
        ctx.move_to(gx - le.width / 2 - le.x_bearing, gy + gr + 32)
        ctx.show_text(g["name"])

    # Telemetry data list
    draw_rounded_rect(ctx, c2_x + 35, c2_y + 320, c2_w - 70, 210, 16)
    ctx.set_source_rgba(0.12, 0.12, 0.16, 0.7 * alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(0.2, 0.2, 0.25, 0.4 * alpha)
    ctx.set_line_width(1.0)
    ctx.stroke()

    tmetrics = [
        ("NVIDIA RTX 3060 Laptop", "48°C / 42W"),
        ("12th Gen Intel Core i7", "4.4 GHz (16T)"),
        ("Active Memory Footprint", "38 MB daemon RAM"),
        ("IPC Protocol", "UNIX Domain Socket"),
    ]
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(15.0)
    for i, (k, v) in enumerate(tmetrics):
        ctx.set_source_rgba(0.65, 0.65, 0.70, alpha)
        ctx.move_to(c2_x + 55, c2_y + 365 + i * 42)
        ctx.show_text(k)

        ve = ctx.text_extents(v)
        ctx.set_source_rgba(0.95, 0.95, 0.98, alpha)
        ctx.move_to(c2_x + c2_w - 55 - ve.width - ve.x_bearing, c2_y + 365 + i * 42)
        ctx.show_text(v)

    # 3. Flint Spectrum Visualizer (Right)
    c3_x = 1280
    c3_y = 220 + slide_y
    c3_w = 460
    c3_h = 580
    draw_rounded_rect(ctx, c3_x, c3_y, c3_w, c3_h, 28)
    ctx.set_source_rgba(0.08, 0.08, 0.10, 0.85 * alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(0.22, 0.22, 0.28, 0.6 * alpha)
    ctx.set_line_width(1.2)
    ctx.stroke()

    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(14.0)
    ctx.set_source_rgba(0.5, 0.5, 0.55, alpha)
    ctx.move_to(c3_x + 30, c3_y + 40)
    ctx.show_text("SKIN: FLINT / AUDIO VISUALIZER")

    # Animated 24-Band Spectrum Bars
    num_bars = 20
    bar_w = 14
    bar_spacing = 6
    total_bars_w = num_bars * bar_w + (num_bars - 1) * bar_spacing
    bar_start_x = c3_x + (c3_w - total_bars_w) / 2
    bar_base_y = c3_y + 330

    for b in range(num_bars):
        bx = bar_start_x + b * (bar_w + bar_spacing)
        # Dynamic animated heights simulating music FFT
        freq = (b + 1) * 0.8
        bh = 20 + 130 * (0.5 + 0.5 * math.sin(local_t * 6.0 + b * 0.45) * math.cos(local_t * 3.0 + b * 0.2))
        
        draw_rounded_rect(ctx, bx, bar_base_y - bh, bar_w, bh, 6)
        b_grad = cairo.LinearGradient(bx, bar_base_y - bh, bx, bar_base_y)
        b_grad.add_color_stop_rgba(0.0, 1.0, 0.4, 0.6, alpha)
        b_grad.add_color_stop_rgba(1.0, 0.04, 0.52, 1.0, alpha)
        ctx.set_source(b_grad)
        ctx.fill()

    # MPRIS Media Info Card
    draw_rounded_rect(ctx, c3_x + 35, c3_y + 380, c3_w - 70, 150, 16)
    ctx.set_source_rgba(0.12, 0.12, 0.16, 0.7 * alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(0.2, 0.2, 0.25, 0.4 * alpha)
    ctx.set_line_width(1.0)
    ctx.stroke()

    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(18.0)
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha)
    ctx.move_to(c3_x + 55, c3_y + 425)
    ctx.show_text("MPRIS2 Integration")

    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(14.0)
    ctx.set_source_rgba(0.65, 0.65, 0.70, alpha)
    ctx.move_to(c3_x + 55, c3_y + 460)
    ctx.show_text("PipeWire audio capture + real-time FFT")
    ctx.move_to(c3_x + 55, c3_y + 490)
    ctx.show_text("Zero CPU usage when audio is idle")

    ctx.restore()

def render_scene_4(ctx, frame: int, t_sec: float):
    """Scene 4: Resolution & Call to Action (12.0s - 15.0s)"""
    local_t = t_sec - 12.0
    t_in = min(1.0, local_t / 1.0)
    ease = cubic_bezier_apple(t_in)
    
    alpha_out = 1.0
    if local_t > 2.2:
        alpha_out = max(0.0, 1.0 - (local_t - 2.2) / 0.8)
    
    alpha = ease * alpha_out
    slide_y = (1.0 - ease) * 30.0

    ctx.save()

    # Final Hero Typography
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(78.0)
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha)
    hero = "Pluvia"
    ext = ctx.text_extents(hero)
    ctx.move_to((WIDTH - ext.width) / 2 - ext.x_bearing, HEIGHT / 2 - 120 + slide_y)
    ctx.show_text(hero)

    # Subline
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(26.0)
    ctx.set_source_rgba(0.53, 0.53, 0.55, alpha)
    sub = "Available now for Linux. Open source under MIT."
    sext = ctx.text_extents(sub)
    ctx.move_to((WIDTH - sext.width) / 2 - sext.x_bearing, HEIGHT / 2 - 55 + slide_y)
    ctx.show_text(sub)

    # Terminal installation box
    term_w = 680
    term_h = 100
    term_x = (WIDTH - term_w) / 2
    term_y = HEIGHT / 2 + 10 + slide_y

    draw_rounded_rect(ctx, term_x, term_y, term_w, term_h, 20)
    ctx.set_source_rgba(0.08, 0.08, 0.11, 0.95 * alpha)
    ctx.fill_preserve()
    ctx.set_source_rgba(0.25, 0.25, 0.32, 0.8 * alpha)
    ctx.set_line_width(1.2)
    ctx.stroke()

    # Terminal command
    ctx.select_font_face("DejaVu Sans Mono", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD)
    ctx.set_font_size(22.0)
    
    ctx.set_source_rgba(0.04, 0.52, 1.0, alpha) # Blue prompt
    ctx.move_to(term_x + 35, term_y + 58)
    ctx.show_text("$ ")

    ctx.set_source_rgba(0.95, 0.95, 0.98, alpha)
    ctx.show_text("cargo install pluvia-cli pluvia-daemon")

    # GitHub Repository Link
    ctx.select_font_face("Adwaita Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_NORMAL)
    ctx.set_font_size(20.0)
    ctx.set_source_rgba(0.45, 0.45, 0.50, alpha)
    gh = "github.com/DRNZY/pluvia"
    gext = ctx.text_extents(gh)
    ctx.move_to((WIDTH - gext.width) / 2 - gext.x_bearing, HEIGHT / 2 + 175 + slide_y)
    ctx.show_text(gh)

    ctx.restore()

def main():
    print(f"[Apple Product Film] Rendering {TOTAL_FRAMES} frames ({DURATION_SECS}s @ {FPS}fps 1080p)...")
    
    out_mp4 = "/home/darnell/Projects/pluvia/brag-output/pluvia_apple_launch.mp4"
    out_video_only = "/home/darnell/Projects/pluvia/brag-output/video_temp.mp4"

    ffmpeg_cmd = [
        "ffmpeg", "-y",
        "-f", "rawvideo",
        "-vcodec", "rawvideo",
        "-s", f"{WIDTH}x{HEIGHT}",
        "-pix_fmt", "bgra",
        "-r", str(FPS),
        "-i", "-",
        "-c:v", "libx264",
        "-preset", "medium",
        "-crf", "17",
        "-pix_fmt", "yuv420p",
        out_video_only
    ]

    process = subprocess.Popen(ffmpeg_cmd, stdin=subprocess.PIPE)

    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, WIDTH, HEIGHT)
    ctx = cairo.Context(surface)

    for frame in range(TOTAL_FRAMES):
        t_sec = frame / float(FPS)
        t_norm = frame / float(TOTAL_FRAMES)

        # Clear & draw studio background
        draw_studio_background(ctx, t_norm)

        # Route scenes
        if t_sec < 3.8:
            render_scene_1(ctx, frame, t_sec)
        elif t_sec < 7.5:
            render_scene_2(ctx, frame, t_sec)
        elif t_sec < 12.0:
            render_scene_3(ctx, frame, t_sec)
        else:
            render_scene_4(ctx, frame, t_sec)

        surface.flush()
        data = surface.get_data()
        process.stdin.write(data)

        if frame % 120 == 0:
            print(f"  Frame {frame}/{TOTAL_FRAMES} ({int(frame/TOTAL_FRAMES*100)}%) rendered...")

    process.stdin.close()
    process.wait()
    print("[Apple Product Film] Video stream complete. Synthesizing Apple-grade audio bed...")

    # Synthesize clean, minimalist Apple-style cinematic audio bed (warm sub-bass drone, soft synth chord swell, tactile UI ticks)
    # Using ffmpeg audio filter graph
    audio_cmd = [
        "ffmpeg", "-y",
        "-i", out_video_only,
        "-f", "lavfi", "-i", "sine=frequency=55:duration=15", # Warm sub-bass foundation
        "-f", "lavfi", "-i", "sine=frequency=110:duration=15", # Mid bass
        "-f", "lavfi", "-i", "sine=frequency=220:duration=15", # Warm synth pad
        "-filter_complex",
        "[1:a]volume=0.25,lowpass=f=120,afade=t=in:ss=0:d=1.5,afade=t=out:st=13.5:d=1.5[a1];"
        "[2:a]volume=0.12,lowpass=f=250,afade=t=in:ss=0:d=2.0,afade=t=out:st=13.0:d=2.0[a2];"
        "[3:a]volume=0.06,lowpass=f=400,afade=t=in:ss=1.0:d=2.5,afade=t=out:st=12.5:d=2.5[a3];"
        "[a1][a2][a3]amix=inputs=3:duration=first:dropout_transition=2,volume=1.8[aout]",
        "-map", "0:v",
        "-map", "[aout]",
        "-c:v", "copy",
        "-c:a", "aac",
        "-b:a", "192k",
        "-shortest",
        out_mp4
    ]

    subprocess.run(audio_cmd, check=True)
    subprocess.run(["rm", "-f", out_video_only])

    # Also copy to root brag.mp4 in brag-output for immediate drop-in replacement
    subprocess.run(["cp", out_mp4, "/home/darnell/Projects/pluvia/brag-output/brag.mp4"])
    print(f"[Apple Product Film] Successfully rendered to {out_mp4}")

if __name__ == "__main__":
    main()
