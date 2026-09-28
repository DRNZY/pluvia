//! Offline skin render harness: parses a skin, ticks measures, renders a PNG, and
//! prints layout diagnostics. Used for verifying layout/scale/sizing fixes headlessly.
//!
//! Usage: cargo run -p pluvia-core --example render_skin -- <skin.ini> <out.png> [WxH]

use cairo::{Format, ImageSurface};
use pluvia_core::ini::parse_skin_file;
use pluvia_core::measures::{create_measure, Measure, MeasureValue};
use pluvia_core::render::meter_renderer::{MeterRenderer, SkinState};
use pluvia_core::render::pango_text::FontRegistry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: render_skin <skin.ini> <out.png> [WxH] [scale]");
        std::process::exit(2);
    }
    let ini = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    let forced: Option<(i32, i32)> = args.get(3).and_then(|s| {        let mut it = s.split(['x', 'X']);
        let w = it.next()?.parse().ok()?;
        let h = it.next()?.parse().ok()?;
        Some((w, h))
    });

    let config = match parse_skin_file(&ini) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("parse error: {e}");
            std::process::exit(1);
        }
    };

    let registered = FontRegistry::register_skin_fonts(&config.skin_dir);
    eprintln!("fonts registered: {registered}");

    let mut measures: HashMap<String, Box<dyn Measure>> = HashMap::new();
    let mut order: Vec<String> = config.measure_order.clone();
    for (name, mc) in &config.measures {
        if let Some(m) = create_measure(mc) {
            measures.insert(name.clone(), m);
        } else {
            eprintln!("! no implementation for measure {name} (type={})", mc.measure_type);
        }
    }
    order.sort();

    let mut values: HashMap<String, MeasureValue> = HashMap::new();
    for name in &order {
        if let Some(m) = measures.get_mut(name) {
            let v = m.update_with_context(&config.variables, &values);
            values.insert(name.clone(), v);
        }
    }
    for (k, v) in &values {
        eprintln!("measure {k} = {v:?}");
    }

    // Same window-size derivation as pluvia-daemon SkinRuntime::calculate_bounds.
    let (mut max_w, mut max_h) = (0.0f64, 0.0f64);
    for name in &config.meter_order {
        let meter = match config.meters.get(name) {
            Some(m) => m,
            None => continue,
        };
        let num = |raw: Option<&String>| -> f64 {
            raw.map(|s| config.variables.expand(s))
                .and_then(|s| {
                    pluvia_core::formulas::eval_formula(&s, &config.variables)
                        .ok()
                        .or_else(|| s.trim().parse::<f64>().ok())
                })
                .unwrap_or(0.0)
        };
        let (x, y) = (
            num(meter.properties.get("x")),
            num(meter.properties.get("y")),
        );
        let (w, h) = (meter.w.unwrap_or(0.0), meter.h.unwrap_or(0.0));
        let eff_w = if w > 0.0 { w } else { 100.0 };
        let eff_h = if h > 0.0 { h } else { 40.0 };
        let align = meter
            .get("stringalign")
            .map(pluvia_core::render::TextAlign::parse)
            .unwrap_or(pluvia_core::render::TextAlign::Left);
        let right = match align {
            pluvia_core::render::TextAlign::Center => (x + eff_w / 2.0).max(eff_w).max(x),
            pluvia_core::render::TextAlign::Right => x.max(eff_w),
            _ => x + eff_w,
        };
        max_w = max_w.max(right);
        max_h = max_h.max(y + eff_h);
    }
    if let Some(sec) = config.raw_sections.get("rainmeter") {
        if let Some(sw) = sec.get("windoww").and_then(|v| v.parse::<f64>().ok()) {
            max_w = max_w.max(sw);
        }
        if let Some(sh) = sec.get("windowh").and_then(|v| v.parse::<f64>().ok()) {
            max_h = max_h.max(sh);
        }
    }
    let (auto_w, auto_h) = (max_w.ceil() as u32, max_h.ceil() as u32);
    let renderer = MeterRenderer::new();
    let state = SkinState::from_arc(Arc::new(config), Arc::new(values));
    let content = renderer.measure_content(&state);
    eprintln!(
        "content rect = x={:.1} y={:.1} w={:.1} h={:.1}",
        content.x, content.y, content.width, content.height
    );
    let need_w = (max_w.max(content.x + content.width)).ceil() as i32;
    let need_h = (max_h.max(content.y + content.height)).ceil() as i32;
    let (win_w, win_h) = forced.unwrap_or((need_w.max(200), need_h.max(180)));
    eprintln!("auto bounds = {auto_w}x{auto_h}; using {win_w}x{win_h}");
    let surface = ImageSurface::create(Format::ARgb32, win_w, win_h)
        .expect("failed to allocate surface");
    let hit = renderer
        .render_to_surface(&state, &surface)
        .expect("render failed");

    let mut file = std::fs::File::create(&out).expect("png create failed");
    surface.write_to_png(&mut file).expect("png write failed");
    eprintln!("wrote {}", out.display());
    eprintln!("hit rects: {}", hit.to_rectangles().len());
    for r in hit.to_rectangles() {
        eprintln!("  hit {:?} {:?} {:?} {:?}", r.x, r.y, r.width, r.height);
    }
}
