//! Capture this application's own renderer output, using fictional settings only.
//! cargo run --example render_native --target x86_64-pc-windows-msvc -- <output.ppm> [skia|femtovg]
use slint::ComponentHandle;
use std::{error::Error, io::Write, path::PathBuf, time::Duration};
slint::include_modules!();
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Supply a PPM output path")?);
    let renderer = args.next().unwrap_or_else(|| "skia".into());
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.clone())
        .select()?;
    let ui = AppWindow::new()?;
    let captured = std::rc::Rc::new(std::cell::Cell::new(false));
    let captured_in_callback = captured.clone();
    ui.set_current_page(Page::Settings);
    ui.set_llm_model("qwen3:4b-thinking".into());
    ui.set_embeddings_model("nomic-embed-text-v2-moe:latest".into());
    ui.set_llm_status("Ollama offline · queue saved".into());
    ui.set_search_limit(10);
    ui.set_sync_address("192.168.0.105:8743".into());
    ui.set_sync_status("Server stopped".into());
    ui.set_sync_token("fictional-preview-key".into());
    let code = qrcode::QrCode::new("Layout fixture - not a pairing secret")?;
    let svg = code.render::<qrcode::render::svg::Color>().build();
    ui.set_sync_qr(slint::Image::load_from_svg_data(svg.as_bytes())?);
    ui.window().set_size(slint::LogicalSize::new(1180.0, 820.0));
    let preview = ui.clone_strong();
    let path = output.clone();
    slint::Timer::single_shot(Duration::from_secs(1), move || {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let pixels = preview.window().take_snapshot()?;
            if pixels
                .as_slice()
                .iter()
                .all(|p| p.r == 0 && p.g == 0 && p.b == 0)
            {
                return Err("Renderer returned an empty black frame".into());
            }
            let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
            write!(file, "P6\n{} {}\n255\n", pixels.width(), pixels.height())?;
            for pixel in pixels.as_slice() {
                file.write_all(&[pixel.r, pixel.g, pixel.b])?;
            }
            file.flush()?;
            captured_in_callback.set(true);
            println!(
                "Captured {renderer} at {}×{}, scale {}",
                pixels.width(),
                pixels.height(),
                preview.window().scale_factor()
            );
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Renderer capture failed: {error}");
        }
        let _ = slint::quit_event_loop();
    });
    ui.run()?;
    if !captured.get() {
        return Err("Renderer did not produce the snapshot".into());
    }
    Ok(())
}
