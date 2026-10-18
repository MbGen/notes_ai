//! Render the real Slint screens offscreen using fictional, in-memory content.
//! cargo run --example render_previews -- <output-directory> [width] [height]
use slint::platform::{
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    Platform, WindowAdapter,
};
use slint::{ComponentHandle, ModelRc, Rgb8Pixel, SharedPixelBuffer, VecModel};
use std::{error::Error, io::Write, rc::Rc};
slint::include_modules!();
struct Offscreen(Rc<MinimalSoftwareWindow>);
impl Platform for Offscreen {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn render(
    ui: &AppWindow,
    window: &MinimalSoftwareWindow,
    output: &std::path::Path,
    name: &str,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn Error>> {
    ui.window()
        .set_size(slint::PhysicalSize::new(width, height));
    ui.show()?;
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let mut pixels = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(pixels.make_mut_slice(), width as usize);
    }));
    let mut file = std::fs::File::create(output.join(format!("{name}.ppm")))?;
    write!(file, "P6\n{width} {height}\n255\n")?;
    file.write_all(pixels.as_bytes())?;
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let output = std::path::PathBuf::from(args.get(1).ok_or("Supply output directory")?);
    let width = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(1180);
    let height = args.get(3).and_then(|x| x.parse().ok()).unwrap_or(820);
    std::fs::create_dir_all(&output)?;
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Offscreen(window.clone())))?;
    let ui = AppWindow::new()?;
    ui.set_note_total(24);
    ui.set_category_total(6);
    ui.set_new_total(3);
    ui.set_llm_running(true);
    ui.set_llm_status("Ollama connected".into());
    ui.set_sync_running(true);
    ui.set_sync_status("Listening on LAN, port 8743".into());
    ui.set_sync_address("192.168.1.10:8743".into());
    ui.set_sync_enabled(true);
    ui.set_sync_devices("Pixel 9a  ·  online".into());
    ui.set_llm_model("qwen3:4b-thinking".into());
    ui.set_embeddings_model("nomic-embed-text-v2-moe:latest".into());
    ui.set_search_limit(10);
    // Fixture key, not credentials. Never read the user's notes or settings.
    ui.set_sync_token("preview-key-not-a-real-secret".into());
    let svg = qrcode::QrCode::new("Notes AI layout preview")?
        .render::<qrcode::render::svg::Color>()
        .build();
    ui.set_sync_qr(slint::Image::load_from_svg_data(svg.as_bytes())?);
    ui.set_last_saved_category("Places & experiences".into());
    ui.set_last_saved_text(
        "A quiet weekend by the sea. Save the small hotel with the courtyard.".into(),
    );
    ui.set_main_notes(ModelRc::new(VecModel::from(vec![NoteData{id:1,text:"The best ideas arrive when there is room to think. Leave a little space in tomorrow's calendar.".into(),date:"Awaiting AI".into()}])));
    render(&ui, &window, &output, "atelier-capture", width, height)?;
    let names = [
        ("Ideas & inspiration", 7, 2),
        ("Places & experiences", 5, 1),
        ("Books worth reading", 4, 0),
        ("Work & craft", 3, 0),
        ("Little reminders", 3, 0),
        ("Films for a rainy day", 2, 0),
    ];
    let tiles = names
        .iter()
        .map(|(name, count, new_count)| ClassTileData {
            name: (*name).into(),
            count: format!("{count} notes").into(),
            new_count: *new_count,
            accent: Default::default(),
        })
        .collect::<Vec<_>>();
    let rows = tiles
        .chunks(3)
        .map(|row| TileRow {
            t0: row[0].clone(),
            t1: row[1].clone(),
            t2: row[2].clone(),
            len: 3,
        })
        .collect::<Vec<_>>();
    ui.set_rows(ModelRc::new(VecModel::from(rows)));
    ui.set_current_page(Page::Classes);
    render(&ui, &window, &output, "atelier-library", width, height)?;
    ui.set_current_page(Page::ClassDetail);
    ui.set_selected_class("Ideas & inspiration".into());
    ui.set_class_notes(ModelRc::new(VecModel::from(vec![
        NoteData {
            id: 1,
            text: "Build fewer things, with more care. A useful reminder for the next release."
                .into(),
            date: "18 October 2026".into(),
        },
        NoteData {
            id: 2,
            text: "A reading corner, a warm lamp, and a place for ideas to settle.".into(),
            date: "17 October 2026".into(),
        },
    ])));
    render(&ui, &window, &output, "atelier-collection", width, height)?;
    ui.set_current_page(Page::SearchResults);
    ui.set_web_mode(true);
    ui.set_query("the art of paying attention".into());
    ui.set_search_message("1 source found · layout fixture".into());
    ui.set_web_results(ModelRc::new(VecModel::from(vec![WebResultData{title:"A little more space for what matters".into(),url:"https://example.org/reading".into(),domain:"example.org".into(),snippet:"A fictional source used to preview the reading experience. Real searches show the provider's titles, descriptions and source links.".into(),saved:false}])));
    render(&ui, &window, &output, "atelier-discover", width, height)?;
    ui.set_current_page(Page::Settings);
    render(&ui, &window, &output, "atelier-preferences", width, height)?;
    println!("Rendered five Slint screens at {width}×{height}");
    Ok(())
}
