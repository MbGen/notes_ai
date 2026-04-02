#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use core::time;
use std::{error::Error};
use std::rc::Rc;
use slint::{Model, Timer};
use slint::{Color, ModelRc, SharedString, VecModel};
use chrono::Local;
use tokio;

mod db;
mod agent;
mod settings;
use db::Database;
use agent::{SortingAgentImpl, OllamaInstance};
use settings::Settings;

slint::include_modules!();

enum AgentRequest {
    Search { text: String, limit: u32 },
    Embed { note_id: i64, text: String },
}

// ── Tile packing ──────────────────────────────────────────────────────────────
fn add_tile(rows: &Rc<VecModel<TileRow>>, tile: ClassTileData) {
    let n = rows.row_count();
    if n == 0 {
        rows.push(TileRow { t0: tile, t1: Default::default(), t2: Default::default(), len: 1 });
        return;
    }
    let last = rows.row_data(n - 1).unwrap();
    if last.len < 3 {
        let updated = match last.len {
            1 => TileRow { t0: last.t0, t1: tile, t2: Default::default(), len: 2 },
            2 => TileRow { t0: last.t0, t1: last.t1, t2: tile,            len: 3 },
            _ => last,
        };
        rows.set_row_data(n - 1, updated);
    } else {
        rows.push(TileRow { t0: tile, t1: Default::default(), t2: Default::default(), len: 1 });
    }
}

fn setup_settings_callback(ui: &AppWindow) {
    let ui_handle = ui.as_weak();
    ui.on_save_settings(move || {
        let Some(ui) = ui_handle.upgrade() else { return };
        let s = Settings {
            llm_model: ui.get_llm_model().to_string(),
            embeddings_model: ui.get_embeddings_model().to_string(),
            search_limit: ui.get_search_limit() as u32,
        };
        s.save().unwrap_or_else(|e| println!("Failed to save settings: {e}"));
    });
}

fn setup_note_callbacks(
    ui: &AppWindow,
    db: Rc<Database>,
    agent_tx: tokio::sync::mpsc::Sender<AgentRequest>,
) {
    ui.on_add_class_note({
        let db = db.clone();
        let agent_tx = agent_tx.clone();
        move |text, class_name| {
            let cats = db.get_categories().unwrap();
            let Some((cat_id, _)) = cats.iter().find(|(_, c)| *c == class_name.to_string()) else { return };
            let today = Local::now().format("%d %B %Y").to_string();
            let note_id = db.insert_note(*cat_id, text.as_str(), &today).unwrap();
            let _ = agent_tx.try_send(AgentRequest::Embed { note_id, text: text.to_string() });
        }
    });

    ui.on_delete_class_note({
        let db = db.clone();
        move |index| {
            db.delete_note(index as i64).unwrap_or_else(|_| println!("Cannot delete note {}", index));
            db.delete_embeddings(index as i64).unwrap();
        }
    });

    ui.on_delete_class_wrapper({
        let db = db.clone();
        move |class_name| {
            let cats = db.get_categories().unwrap();
            let Some((cat_id, _)) = cats.iter().find(|(_, c)| *c == class_name.to_string()) else { return };
            db.delete_category(*cat_id).unwrap();
        }
    });

    ui.on_rename_class_wrapper({
        let db = db.clone();
        move |old_name, new_name| {
            let cats = db.get_categories().unwrap();
            let Some((cat_id, _)) = cats.iter().find(|(_, c)| c.as_str() == old_name.as_str()) else { return };
            db.rename_category(*cat_id, new_name.as_str()).unwrap();
        }
    });
}

fn setup_queue_callbacks(ui: &AppWindow, db: Rc<Database>) {
    ui.on_add_main_note({
        let db = db.clone();
        move |text| {
            if text.trim().is_empty() { return; }
            db.insert_to_queue(text.as_str()).unwrap();
        }
    });

    ui.on_delete_main_note({
        let db = db.clone();
        move |index| {
            db.delete_queue_item(index as i64)
                .unwrap_or_else(|_| println!("Cannot delete queue item {}", index));
        }
    });
}

fn setup_search_callback(
    ui: &AppWindow,
    agent_tx: tokio::sync::mpsc::Sender<AgentRequest>,
) {
    let ui_handle = ui.as_weak();
    ui.on_on_semantic_search_wrapper(move |text| {
        if let Some(ui) = ui_handle.upgrade() {
            ui.set_search_results(ModelRc::from(Rc::new(VecModel::<NoteData>::default())));
            ui.set_current_page(Page::SearchResults);
        }
        let limit = ui_handle.upgrade().map(|u| u.get_search_limit() as u32).unwrap_or(10);
        let _ = agent_tx.try_send(AgentRequest::Search { text: text.to_string(), limit });
    });
}

fn setup_timers(
    ui: &AppWindow,
    db: Rc<Database>,
    rows: Rc<VecModel<TileRow>>,
    notes: Rc<VecModel<NoteData>>,
    main_notes: Rc<VecModel<NoteData>>,
) -> (Timer, Timer, Timer) {
    let categories_timer = Timer::default();
    categories_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(2),
        {
            let db = db.clone();
            move || {
                let categories = db.get_categories().unwrap();
                rows.set_vec(vec![]);
                categories.iter().for_each(|(id, name)| {
                    add_tile(&rows, ClassTileData {
                        name: name.as_str().into(),
                        count: format!("{} notes", db.get_notes(*id).unwrap().len()).into(),
                        accent: Color::from_rgb_u8(0x59, 0x24, 0x2d),
                    });
                });
            }
        },
    );

    let ui_handle = ui.as_weak();
    let class_notes_timer = slint::Timer::default();
    class_notes_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(500),
        {
            let db = db.clone();
            move || {
                let Some(ui) = ui_handle.upgrade() else { return };
                if ui.get_current_page() != Page::ClassDetail { return; }
                let class_name = ui.get_selected_class();
                let cats = db.get_categories().unwrap();
                let Some(cat_id) = cats.iter().find(|(_, n)| n.as_str() == class_name.as_str()).map(|(id, _)| *id) else { return };
                notes.set_vec(
                    db.get_notes(cat_id).unwrap().iter()
                        .map(|(id, text, date)| NoteData { id: *id as i32, text: text.as_str().into(), date: date.as_str().into() })
                        .collect::<Vec<_>>()
                );
            }
        },
    );

    let queue_timer = slint::Timer::default();
    queue_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(2),
        {
            let db = db.clone();
            move || {
                let items = db.get_queue_items().unwrap();
                main_notes.set_vec(
                    items.iter().map(|(id, text)| NoteData {
                        id: *id as i32,
                        text: text.as_str().into(),
                        date: SharedString::new(),
                    }).collect::<Vec<_>>()
                );
            }
        },
    );

    (categories_timer, class_notes_timer, queue_timer)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;
    let db = Rc::new(Database::new("data/notes.db"));
    db.init_schemas()?;

    let settings = Settings::load();
    ui.set_llm_model(settings.llm_model.as_str().into());
    ui.set_embeddings_model(settings.embeddings_model.as_str().into());
    ui.set_search_limit(settings.search_limit as i32);

    // Ensure settings file exists on disk
    let ui_alert = ui.as_weak();
    Settings::save(&Settings::default()).unwrap_or_else(|_| {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_alert.upgrade() {
                ui.set_alert_message(format!("Cannot save setting by path {}", settings::SETTINGS_PATH).into());
                ui.set_alert_visible(true);
            }
        });
    });

    let (agent_tx, mut agent_rx) = tokio::sync::mpsc::channel::<AgentRequest>(10);

    // Background LLM task
    let ui_handle = ui.as_weak();
    let spawn_settings = Settings::load();
    tokio::spawn(async move {
        let sorting_agent = match SortingAgentImpl::new(
            OllamaInstance::new(&spawn_settings.llm_model, &spawn_settings.embeddings_model)
        ).await {
            Ok(agent) => agent,
            Err(msg)  => {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_alert_message(msg.into());
                        ui.set_alert_visible(true);
                    }
                });
                return;
            }
        };
        let db = Database::new("data/notes.db");
        loop {
            tokio::select! {
                Some(request) = agent_rx.recv() => {
                    match request {
                        AgentRequest::Search { text, limit } => {
                            if let Ok(embeddings) = sorting_agent.get_embeddings(&text).await {
                                let results = db.search_by_embeddings(embeddings, limit);
                                let ui_handle = ui_handle.clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_handle.upgrade() {
                                        let notes: Vec<NoteData> = results.iter().enumerate().map(|(i, text)| NoteData {
                                            id: i as i32,
                                            text: text.as_str().into(),
                                            date: SharedString::new(),
                                        }).collect();
                                        ui.set_search_results(ModelRc::from(Rc::new(VecModel::from(notes))));
                                        ui.set_current_page(Page::SearchResults);
                                    }
                                });
                            }
                        }
                        AgentRequest::Embed { note_id, text } => {
                            if let Ok(embeddings) = sorting_agent.get_embeddings(&text).await {
                                db.insert_embeddings(note_id, embeddings).unwrap();
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(time::Duration::from_secs(1)) => {
                    let queue_notes = db.get_queue_items().unwrap();
                    if queue_notes.is_empty() { continue; }

                    println!("Queue notes left {}", queue_notes.len());
                    let available_classes = db.get_categories().unwrap();
                    let class_names: Vec<String> = available_classes.iter().map(|(_, name)| name.clone()).collect();
                    let today = Local::now().format("%d %B %Y").to_string();

                    for (queue_id, note_text) in queue_notes {
                        let Some(class_name) = sorting_agent.classify_note(class_names.clone(), note_text.as_str()).await else { continue };

                        let lower = class_name.to_lowercase();
                        let class_id = match available_classes.iter().find(|(_, c)| c.to_lowercase() == lower) {
                            Some((id, _)) => *id,
                            None          => db.insert_category(&class_name).unwrap(),
                        };

                        let note_id = db.insert_note(class_id, &note_text, &today).unwrap();
                        let embeddings = sorting_agent.get_embeddings(&note_text).await.unwrap();
                        db.insert_embeddings(note_id, embeddings).unwrap();
                        db.delete_queue_item(queue_id).unwrap();
                    }
                }
            }
        }
    });

    let rows: Rc<VecModel<TileRow>> = Rc::new(VecModel::default());
    let notes: Rc<VecModel<NoteData>> = Rc::new(VecModel::default());
    let main_notes: Rc<VecModel<NoteData>> = Rc::new(VecModel::default());

    ui.set_rows(ModelRc::from(rows.clone()));
    ui.set_class_notes(ModelRc::from(notes.clone()));
    ui.set_main_notes(ModelRc::from(main_notes.clone()));

    setup_settings_callback(&ui);
    setup_note_callbacks(&ui, db.clone(), agent_tx.clone());
    setup_queue_callbacks(&ui, db.clone());
    setup_search_callback(&ui, agent_tx);

    // Timers must stay alive until ui.run() returns
    let _timers = setup_timers(&ui, db, rows, notes, main_notes);

    ui.run()?;
    Ok(())
}
