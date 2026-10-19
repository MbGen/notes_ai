#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use slint::{Color, Model, ModelRc, SharedString, Timer, VecModel};
use std::{
    cell::RefCell,
    error::Error,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
mod agent;
mod data_transfer;
mod db;
mod settings;
mod note_order;
mod sync;
mod web_search;
use agent::{OllamaInstance, SortingAgentImpl};
use db::Database;
use settings::Settings;
slint::include_modules!();

enum AgentRequest {
    Search {
        text: String,
        limit: u32,
        generation: u64,
    },
    Embed {
        note_id: i64,
        text: String,
    },
}

#[derive(Default)]
struct ViewState {
    categories: Option<Vec<(i64, String, i64, i64)>>,
    queue: Option<Vec<(i64, String)>>,
    notes: Option<(String, Vec<(i64, String, String)>)>,
    last_id: i64,
}

fn alert(ui: &AppWindow, message: impl Into<String>) {
    ui.set_alert_success(false);
    ui.set_alert_message(message.into().into());
    ui.set_alert_visible(true);
}

fn note_model(notes: &[(i64, String, String)], newest: bool) -> ModelRc<NoteData> {
    let mut notes = notes.to_vec();
    note_order::sort(&mut notes, newest);
    ModelRc::new(VecModel::from(
        notes
            .iter()
            .map(|(id, text, date)| NoteData {
                id: *id as i32,
                text: text.as_str().into(),
                date: date.as_str().into(),
            })
            .collect::<Vec<_>>(),
    ))
}

fn refresh(ui: &AppWindow, db: &Database, view: &mut ViewState) -> rusqlite::Result<()> {
    let categories = db.get_categories()?;
    if ui.get_current_page() == Page::ClassDetail {
        let name = ui.get_selected_class().to_string();
        if let Some((id, _)) = categories.iter().find(|(_, n)| n == &name) {
            db.mark_category_seen(*id)?;
            let notes = db.get_notes(*id)?;
            if view.notes.as_ref() != Some(&(name.clone(), notes.clone())) {
                ui.set_class_notes(note_model(&notes, ui.get_notes_newest_first()));
                view.notes = Some((name, notes));
            }
        } else {
            ui.set_current_page(Page::Classes);
        }
    }
    let stats = db.category_stats()?;
    if view.categories.as_ref() != Some(&stats) {
        ui.set_category_total(stats.len() as i32);
        ui.set_note_total(stats.iter().map(|(_, _, count, _)| count).sum::<i64>() as i32);
        ui.set_new_total(stats.iter().map(|(_, _, _, count)| count).sum::<i64>() as i32);
        let tiles = stats
            .iter()
            .map(|(_, name, count, new_count)| ClassTileData {
                name: name.as_str().into(),
                count: format!("{count} notes").into(),
                accent: Color::from_rgb_u8(0x18, 0x1b, 0x1a),
                new_count: *new_count as i32,
            })
            .collect::<Vec<_>>();
        let rows = tiles
            .chunks(3)
            .map(|row| TileRow {
                t0: row[0].clone(),
                t1: row.get(1).cloned().unwrap_or_default(),
                t2: row.get(2).cloned().unwrap_or_default(),
                len: row.len() as i32,
            })
            .collect::<Vec<_>>();
        ui.set_rows(ModelRc::new(VecModel::from(rows)));
        view.categories = Some(stats);
    }
    let queue = db.get_queue_items()?;
    if view.queue.as_ref() != Some(&queue) {
        ui.set_main_notes(ModelRc::new(VecModel::from(
            queue
                .iter()
                .map(|(id, text)| NoteData {
                    id: *id as i32,
                    text: text.as_str().into(),
                    date: SharedString::new(),
                })
                .collect::<Vec<_>>(),
        )));
        view.queue = Some(queue);
    }
    if let Some((id, category, text)) = db.latest_note()? {
        let notify = id > view.last_id && view.last_id >= 0;
        view.last_id = view.last_id.max(id);
        ui.set_last_saved_id(id as i32);
        ui.set_last_saved_category(category.into());
        ui.set_last_saved_text(text.into());
        if notify {
            ui.set_notice_visible(true);
        }
    } else {
        ui.set_last_saved_category(SharedString::new());
        ui.set_last_saved_text(SharedString::new());
        ui.set_notice_visible(false);
    }
    if view.last_id < 0 {
        view.last_id = 0;
    }
    Ok(())
}

fn update_pairing(ui: &AppWindow, settings: &Settings) {
    let address = (|| -> std::io::Result<std::net::IpAddr> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.connect("192.0.2.1:80")?;
        Ok(socket.local_addr()?.ip())
    })()
    .ok();
    ui.set_sync_address(
        address
            .map(|a| format!("{}:{}", a, settings.sync_port))
            .unwrap_or_else(|| "Connect to Wi-Fi and save preferences".into())
            .into(),
    );
    if let Some(address) = address {
        let payload=serde_json::json!({"type":"notes-ai","version":1,"host":address.to_string(),"port":settings.sync_port,"token":settings.sync_token}).to_string();
        if let Ok(code) = qrcode::QrCode::new(payload) {
            let svg = code
                .render::<qrcode::render::svg::Color>()
                .min_dimensions(240, 240)
                .build();
            if let Ok(image) = slint::Image::load_from_svg_data(svg.as_bytes()) {
                ui.set_sync_qr(image);
            }
        }
    } else {
        ui.set_sync_qr(slint::Image::default());
    }
}

fn wire_settings(
    ui: &AppWindow,
    status: sync::SharedStatus,
    server: Rc<RefCell<tokio::task::JoinHandle<()>>>,
) {
    let handle = ui.as_weak();
    ui.on_save_settings(move || {
        let Some(ui) = handle.upgrade() else { return };
        let port = match ui.get_sync_port().trim().parse::<u16>() {
            Ok(port) if port >= 1024 => port,
            _ => {
                ui.set_sync_enabled(Settings::load().sync_enabled);
                alert(&ui, "Enter a port number between 1024 and 65535.");
                return;
            }
        };
        let settings = Settings {
            llm_model: ui.get_llm_model().trim().to_string(),
            embeddings_model: ui.get_embeddings_model().trim().to_string(),
            search_limit: ui.get_search_limit() as u32,
            classification_web: ui.get_classification_web(),
            sync_enabled: ui.get_sync_enabled(),
            sync_port: port,
            sync_token: ui.get_sync_token().to_string(),
        };
        if settings.llm_model.is_empty() || settings.embeddings_model.is_empty() {
            alert(&ui, "Enter both model names before saving.");
            return;
        }
        if let Err(error) = settings.save() {
            alert(&ui, format!("Cannot save preferences: {error}"));
            return;
        }
        update_pairing(&ui, &settings);
        let previous = std::mem::replace(&mut *server.borrow_mut(), tokio::spawn(async {}));
        previous.abort();
        if let Ok(mut state) = status.lock() {
            *state = sync::SyncStatus::default();
        }
        *server.borrow_mut() = sync::start(settings, status.clone(), Some(previous));
        alert(
            &ui,
            "Preferences saved. AI model changes apply to the next operation.",
        );
        ui.set_alert_success(true);
    });
    let handle = ui.as_weak();
    ui.on_reset_sync_pairing(move || {
        if let Some(ui) = handle.upgrade() {
            ui.set_sync_token(uuid::Uuid::new_v4().to_string().into());
            ui.invoke_save_settings();
        }
    });
}

fn wire_notes(
    ui: &AppWindow,
    db: Rc<Database>,
    agent: tokio::sync::mpsc::UnboundedSender<AgentRequest>,
) {
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_sort_notes(move || {
        if let Some(ui) = handle.upgrade() {
            if let Ok(categories) = store.get_categories() {
                if let Some((id, _)) = categories.iter().find(|(_, name)| name == ui.get_selected_class().as_str()) {
                    if let Ok(notes) = store.get_notes(*id) {
                        ui.set_class_notes(note_model(&notes, ui.get_notes_newest_first()));
                    }
                }
            }
        }
    });
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_add_main_note(move |text| {
        let Some(ui) = handle.upgrade() else { return };
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        match store.insert_to_queue(text) {
            Ok(_) => ui.set_draft(SharedString::new()),
            Err(error) => alert(&ui, format!("Cannot save note: {error}")),
        }
    });
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_delete_main_note(move |id| {
        if let (Some(ui), Err(error)) = (handle.upgrade(), store.delete_queue_item(id as i64)) {
            alert(&ui, format!("Cannot delete queued note: {error}"));
        }
    });
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_delete_class_note(move |id| {
        if let (Some(ui), Err(error)) = (handle.upgrade(), store.delete_note(id as i64)) {
            alert(&ui, format!("Cannot delete note: {error}"));
        }
    });
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_add_class_note(move |text, name| {
        let Some(ui) = handle.upgrade() else { return };
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let result = (|| -> rusqlite::Result<i64> {
            let category = store
                .get_categories()?
                .into_iter()
                .find(|(_, n)| n == name.as_str())
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            store.insert_note(
                category.0,
                text,
                &Local::now().format("%d %B %Y").to_string(),
            )
        })();
        match result {
            Ok(note_id) => {
                ui.set_class_draft(SharedString::new());
                let _ = agent.send(AgentRequest::Embed {
                    note_id,
                    text: text.to_string(),
                });
            }
            Err(error) => alert(&ui, format!("Cannot save note: {error}")),
        }
    });
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_class_opened(move |name| {
        let Some(ui) = handle.upgrade() else { return };
        if let Ok(categories) = store.get_categories() {
            if let Some((id, _)) = categories.iter().find(|(_, n)| n == name.as_str()) {
                if let Err(error) = store.record_category_opened(*id) {
                    alert(&ui,format!("Cannot remember collection order: {error}"));
                }
                let _ = store.mark_category_seen(*id);
                if let Ok(notes) = store.get_notes(*id) {
                    ui.set_class_notes(note_model(&notes, ui.get_notes_newest_first()));
                }
            }
        }
    });
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_delete_class_wrapper(move |name| {
        let Some(ui) = handle.upgrade() else { return };
        let result = (|| -> rusqlite::Result<()> {
            let id = store
                .get_categories()?
                .into_iter()
                .find(|(_, n)| n == name.as_str())
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?
                .0;
            store.delete_category(id)
        })();
        match result {
            Ok(()) => ui.set_current_page(Page::Classes),
            Err(error) => alert(&ui, format!("Cannot delete collection: {error}")),
        }
    });
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_rename_class_wrapper(move |old, new| {
        let Some(ui) = handle.upgrade() else { return };
        let new = new.trim();
        if new.is_empty() {
            return;
        }
        let result = (|| -> rusqlite::Result<()> {
            let categories = store.get_categories()?;
            if categories
                .iter()
                .any(|(_, name)| name.to_lowercase() == new.to_lowercase() && name != old.as_str())
            {
                return Err(rusqlite::Error::InvalidParameterName(
                    "A collection with this name already exists".into(),
                ));
            }
            let id = categories
                .iter()
                .find(|(_, n)| n == old.as_str())
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?
                .0;
            store.rename_category(id, new)
        })();
        match result {
            Ok(()) => {
                ui.set_selected_class(new.into());
                if ui.get_last_saved_category() == old {
                    ui.set_last_saved_category(new.into());
                }
            }
            Err(error) => alert(&ui, format!("Cannot rename collection: {error}")),
        }
    });
    let handle = ui.as_weak();
    ui.on_open_last_note(move || {
        let Some(ui) = handle.upgrade() else { return };
        match db.category_for_note(ui.get_last_saved_id() as i64) {
            Ok(name) => {
                ui.set_selected_class(name.clone().into());
                ui.set_current_page(Page::ClassDetail);
                ui.set_notice_visible(false);
                ui.invoke_class_opened(name.into());
            }
            Err(_) => {
                ui.set_notice_visible(false);
                alert(&ui, "This note has already been removed.");
            }
        }
    });
}

fn begin_search(ui: &AppWindow, query: &str, web: bool, generation: &AtomicU64) -> u64 {
    let id = generation.fetch_add(1, Ordering::SeqCst) + 1;
    ui.set_query(query.into());
    ui.set_web_mode(web);
    ui.set_search_busy(true);
    ui.set_search_message(SharedString::new());
    ui.set_current_page(Page::SearchResults);
    if web {
        ui.set_web_results(ModelRc::new(VecModel::<WebResultData>::default()));
    } else {
        ui.set_search_results(ModelRc::new(VecModel::<NoteData>::default()));
    }
    id
}

fn wire_search(
    ui: &AppWindow,
    db: Rc<Database>,
    agent: tokio::sync::mpsc::UnboundedSender<AgentRequest>,
    generation: Arc<AtomicU64>,
) {
    let handle = ui.as_weak();
    let counter = generation.clone();
    ui.on_on_semantic_search_wrapper(move |query| {
        let Some(ui) = handle.upgrade() else { return };
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        let generation = begin_search(&ui, query, false, &counter);
        if agent
            .send(AgentRequest::Search {
                text: query.to_string(),
                limit: ui.get_search_limit() as u32,
                generation,
            })
            .is_err()
        {
            ui.set_search_busy(false);
            ui.set_search_message("AI search is unavailable. Restart the application.".into());
        }
    });
    let handle = ui.as_weak();
    let counter = generation.clone();
    ui.on_web_search(move |query| {
        let Some(ui) = handle.upgrade() else { return };
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        let id = begin_search(&ui, query, true, &counter);
        let handle = handle.clone();
        let query = query.to_string();
        let counter = counter.clone();
        tokio::spawn(async move {
            let result = web_search::search(&query).await;
            let _ = slint::invoke_from_event_loop(move || {
                if counter.load(Ordering::SeqCst) != id {
                    return;
                }
                if let Some(ui) = handle.upgrade() {
                    ui.set_search_busy(false);
                    match result {
                        Ok(results) => {
                            ui.set_search_message(if results.is_empty() {
                                "No results found. Try a different query.".into()
                            } else {
                                format!("{} sources found", results.len()).into()
                            });
                            ui.set_web_results(ModelRc::new(VecModel::from(
                                results
                                    .into_iter()
                                    .map(|r| WebResultData {
                                        title: r.title.into(),
                                        url: r.url.into(),
                                        domain: r.domain.into(),
                                        snippet: r.snippet.into(),
                                        saved: false,
                                    })
                                    .collect::<Vec<_>>(),
                            )));
                        }
                        Err(error) => ui.set_search_message(error.into()),
                    }
                }
            });
        });
    });
    ui.on_cancel_search(move || {
        generation.fetch_add(1, Ordering::SeqCst);
    });
    let handle = ui.as_weak();
    ui.on_save_web_result(move |index| {
        let Some(ui) = handle.upgrade() else { return };
        let results = ui.get_web_results();
        let Some(mut result) = results.row_data(index as usize) else {
            return;
        };
        if result.saved {
            return;
        }
        if web_search::public_url(result.url.as_str()).is_err() {
            alert(&ui, "This source address is not supported.");
            return;
        }
        let text = format!(
            "{}\n\n{}\n\nSource: {}",
            result.title, result.snippet, result.url
        );
        match db.insert_to_queue(&text) {
            Ok(_) => {
                result.saved = true;
                results.set_row_data(index as usize, result);
            }
            Err(error) => alert(&ui, format!("Cannot save source: {error}")),
        }
    });
    let handle = ui.as_weak();
    ui.on_open_web_result(move |index| {
        if let Some(ui) = handle.upgrade() {
            if let Some(result) = ui.get_web_results().row_data(index as usize) {
                open_source(&ui, result.url.as_str());
            }
        }
    });
    let handle = ui.as_weak();
    ui.on_open_browser_search(move || {
        if let Some(ui) = handle.upgrade() {
            if !ui.get_query().trim().is_empty() {
                if let Ok(url) = web_search::search_url(ui.get_query().as_str()) {
                    open_source(&ui, url.as_str());
                }
            }
        }
    });
}

fn wire_transfer(ui: &AppWindow, db: Rc<Database>, view: Rc<RefCell<ViewState>>) {
    use std::path::Path;
    let pending = Rc::new(RefCell::new(None::<data_transfer::Archive>));
    let handle = ui.as_weak();
    let staged = pending.clone();
    ui.on_import_requested(move || {
        let Some(ui) = handle.upgrade() else { return };
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import Notes AI backup")
            .add_filter("Notes AI backups", &["json", "db", "sqlite", "sqlite3"])
            .pick_file()
        else {
            return;
        };
        match data_transfer::read_archive(&path) {
            Ok(archive) => {
                ui.set_import_preview(archive.summary(&Settings::load().embeddings_model).into());
                *staged.borrow_mut() = Some(archive);
                ui.set_import_confirmation(true);
            }
            Err(error) => alert(&ui, format!("Cannot read backup: {error}")),
        }
    });
    let handle = ui.as_weak();
    let staged = pending.clone();
    ui.on_import_cancelled(move || {
        *staged.borrow_mut() = None;
        if let Some(ui) = handle.upgrade() {
            ui.set_import_confirmation(false);
        }
    });
    let handle = ui.as_weak();
    let store = db.clone();
    ui.on_import_confirmed(move || {
        let Some(ui) = handle.upgrade() else { return };
        ui.set_import_confirmation(false);
        let Some(archive) = pending.borrow_mut().take() else {
            return;
        };
        let result = (|| -> data_transfer::TransferResult<String> {
            let backup = data_transfer::safety_backup(&store, Path::new("data/notes.db"))?;
            let report = data_transfer::import_archive(
                &store,
                &archive,
                &Settings::load().embeddings_model,
            )?;
            view.borrow_mut().last_id = store.latest_note()?.map(|(id, _, _)| id).unwrap_or(0);
            Ok(format!("{report} Safety backup: {}", backup.display()))
        })();
        match result {
            Ok(message) => {
                ui.set_transfer_status(message.clone().into());
                alert(&ui, message);
                ui.set_alert_success(true);
            }
            Err(error) => alert(
                &ui,
                format!("Import failed. Existing notes have been kept: {error}"),
            ),
        }
    });
    let handle = ui.as_weak();
    ui.on_export_data(move |kind| {
        let Some(ui) = handle.upgrade() else { return };
        let extension = match kind.as_str() {
            "db" => "db",
            "md" => "md",
            _ => "json",
        };
        let name = format!(
            "notes-ai-{}.{}",
            Local::now().format("%Y%m%d-%H%M%S"),
            extension
        );
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export Notes AI notes")
            .add_filter("Notes AI export", &[extension])
            .set_file_name(&name)
            .save_file()
        else {
            return;
        };
        let model = Settings::load().embeddings_model;
        let result = match extension {
            "db" => data_transfer::write_sqlite(&db, &path, Path::new("data/notes.db")),
            "md" => data_transfer::write_markdown(&db, &model, &path, Path::new("data/notes.db")),
            _ => data_transfer::write_json(&db, &model, &path, Path::new("data/notes.db")),
        };
        match result {
            Ok(()) => {
                let message = format!("Export saved: {}", path.display());
                ui.set_transfer_status(message.clone().into());
                alert(&ui, message);
                ui.set_alert_success(true);
            }
            Err(error) => alert(&ui, format!("Export failed: {error}")),
        }
    });
}

fn open_source(ui: &AppWindow, raw: &str) {
    match web_search::public_url(raw) {
        Ok(url) => {
            if let Err(error) = open::that_detached(url.as_str()) {
                alert(ui, format!("Cannot open browser: {error}"));
            }
        }
        Err(error) => alert(ui, error),
    }
}

fn ai_progress(handle:&slint::Weak<AppWindow>,message:&str){
    let handle=handle.clone();let message=message.to_string();
    let _=slint::invoke_from_event_loop(move||{if let Some(ui)=handle.upgrade(){ui.set_ai_progress(message.into());}});
}

fn spawn_ai(
    ui: &AppWindow,
    mut requests: tokio::sync::mpsc::UnboundedReceiver<AgentRequest>,
    generation: Arc<AtomicU64>,
) {
    let handle = ui.as_weak();
    tokio::spawn(async move {
        let db = Database::new("data/notes.db");
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        let mut last_queue_attempt = 0;
        loop {
            tokio::select! {
                request=requests.recv()=>{
                    let Some(request)=request else{break};
                    let settings=Settings::load();
                    let agent=SortingAgentImpl::from_models(OllamaInstance::new(&settings.llm_model,&settings.embeddings_model));
                    match request{
                        AgentRequest::Search{text,limit,generation:id}=>{
                            if generation.load(Ordering::SeqCst)!=id{continue;}
                            let result=match agent.get_embeddings(&text).await{
                                Ok(vector)=>db.search_by_embeddings(vector,limit).map_err(|_|"Cannot search the embedding index. Check the embedding model and index dimensions.".to_string()),
                                Err(_)=>Err("Cannot reach the embedding model. Start Ollama and check your model in Preferences.".into()),
                            };
                            let handle=handle.clone();let generation=generation.clone();
                            let _=slint::invoke_from_event_loop(move||{
                                if generation.load(Ordering::SeqCst)!=id{return;}
                                if let Some(ui)=handle.upgrade(){
                                    ui.set_search_busy(false);
                                    match result{
                                        Ok(notes)=>{ui.set_search_message(if notes.is_empty(){"No matching notes yet.".into()}else{format!("{} notes found",notes.len()).into()});let notes=notes.into_iter().map(|(id,text,date,category)|NoteData{id:id as i32,text:text.into(),date:format!("{category} · {date}").into()}).collect::<Vec<_>>();ui.set_search_results(ModelRc::new(VecModel::from(notes)));}
                                        Err(error)=>ui.set_search_message(error.into()),
                                    }
                                }
                            });
                        }
                        AgentRequest::Embed{note_id,text}=>{
                            if let Ok(vector)=agent.get_embeddings(&text).await{
                                // Deletion while embedding is in flight must not leave an orphan index entry.
                                let exists:bool=db.connection().query_row("SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1)",[note_id],|r|r.get(0)).unwrap_or(false);
                                if exists{let _=db.insert_embeddings_if_text(note_id,&text,vector);}
                            }
                        }
                    }
                }
                _=tick.tick()=>{
                    let Ok(queued)=db.get_queue_items() else{continue};
                    if queued.is_empty(){
                        let missing=db.connection().query_row("SELECT n.id,n.text FROM notes n LEFT JOIN note_embeddings e ON e.note_id=n.id WHERE e.note_id IS NULL ORDER BY n.id LIMIT 1",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)));
                        if let Ok((note_id,text))=missing{
                            let settings=Settings::load();let agent=SortingAgentImpl::from_models(OllamaInstance::new(&settings.llm_model,&settings.embeddings_model));
                            if let Ok(vector)=agent.get_embeddings(&text).await{
                                let exists:bool=db.connection().query_row("SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1)",[note_id],|r|r.get(0)).unwrap_or(false);
                                if exists{let _=db.insert_embeddings_if_text(note_id,&text,vector);}
                            }
                        }
                        continue;
                    }
                    // Retry failures fairly: a difficult note must not block every later note.
                    let Some((id,text))=queued.iter().rev().find(|(id,_)|*id>last_queue_attempt).or_else(||queued.last()).cloned() else{continue};
                    last_queue_attempt=id;
                    let settings=Settings::load();let agent=SortingAgentImpl::from_models(OllamaInstance::new(&settings.llm_model,&settings.embeddings_model));
                    let Ok(categories)=db.get_categories() else{continue};
                    ai_progress(&handle,"Analyzing the next note…");
                    let Some(category)=agent.classify_note(categories.into_iter().map(|(_,n)|n).collect(),&text,settings.classification_web,|message|ai_progress(&handle,message)).await else{ai_progress(&handle,"AI unavailable or invalid response · will retry");continue};
                    if category.trim().is_empty(){continue;}
                    ai_progress(&handle,"Building the search index…");
                    if let Ok(vector)=agent.get_embeddings(&text).await{
                        if let Err(error)=db.finish_queue(id,category.trim(),&text,&Local::now().format("%d %B %Y").to_string(),vector){eprintln!("Cannot finish queued note: {error}");}
                        ai_progress(&handle,"");
                    }else{ai_progress(&handle,"Embedding model unavailable · will retry");}
                }
            }
        }
    });
    let handle = ui.as_weak();
    tokio::spawn(async move {
        let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        else {
            return;
        };
        loop {
            let settings = Settings::load();
            let (online, message) = match client.get("http://127.0.0.1:11434/api/tags").send().await
            {
                Ok(response) if response.status().is_success() => match response
                    .json::<serde_json::Value>()
                    .await
                {
                    Ok(body) => {
                        let available = |model: &str| {
                            body["models"]
                                .as_array()
                                .map(|models| {
                                    models.iter().any(|entry| {
                                        entry["name"]
                                            .as_str()
                                            .map(|name| {
                                                name == model || name == format!("{model}:latest")
                                            })
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false)
                        };
                        if available(&settings.llm_model) && available(&settings.embeddings_model) {
                            (true, "Ollama connected")
                        } else {
                            (false, "Install configured models")
                        }
                    }
                    Err(_) => (false, "Cannot read Ollama status"),
                },
                _ => (false, "Ollama offline · queue saved"),
            };
            let handle = handle.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle.upgrade() {
                    ui.set_llm_running(online);
                    ui.set_llm_status(message.into());
                }
            });
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Portable release: keep writable data next to the executable, regardless of
    // the Explorer/shortcut working directory. CLI paths keep their usual meaning.
    if !cfg!(debug_assertions) && std::env::args_os().len()==1 {
        let executable=std::env::current_exe()?;
        std::env::set_current_dir(executable.parent().ok_or("Cannot locate application folder")?)?;
    }
    // Headless maintenance commands use the same validated transfer code as the UI.
    let args = std::env::args().collect::<Vec<_>>();
    if let Some(command) = args.get(1).filter(|s| {
        matches!(
            s.as_str(),
            "--inspect-backup" | "--import-backup" | "--export-backup"
        )
    }) {
        let path = std::path::Path::new(args.get(2).ok_or("Supply a backup file path")?);
        let settings = Settings::load();
        if command == "--inspect-backup" {
            let archive = data_transfer::read_archive(path)?;
            println!(
                "{} notes, {} collections, {} queued, {} vectors",
                archive.notes.len(),
                archive.categories.len(),
                archive.queue.len(),
                archive
                    .notes
                    .iter()
                    .filter(|n| n.embedding.is_some())
                    .count()
            );
            return Ok(());
        }
        let db = Database::new("data/notes.db");
        db.init_schemas()?;
        if command == "--import-backup" {
            let archive = data_transfer::read_archive(path)?;
            let backup = data_transfer::safety_backup(&db, std::path::Path::new("data/notes.db"))?;
            println!("Safety backup: {}", backup.display());
            println!(
                "{}",
                data_transfer::import_archive(&db, &archive, &settings.embeddings_model)?
            );
        } else {
            match path.extension().and_then(|s| s.to_str()) {
                Some("db") => {
                    data_transfer::write_sqlite(&db, path, std::path::Path::new("data/notes.db"))?
                }
                Some("md") => data_transfer::write_markdown(
                    &db,
                    &settings.embeddings_model,
                    path,
                    std::path::Path::new("data/notes.db"),
                )?,
                _ => data_transfer::write_json(
                    &db,
                    &settings.embeddings_model,
                    path,
                    std::path::Path::new("data/notes.db"),
                )?,
            }
            println!("Exported: {}", path.display());
        }
        return Ok(());
    }
    // Keep the crisp Windows renderer when launched directly without a .cmd file.
    slint::BackendSelector::new().backend_name("winit".into())
        .renderer_name("skia".into()).select()?;
    let ui = AppWindow::new()?;
    let db = Rc::new(Database::new("data/notes.db"));
    db.init_schemas()?;
    let settings = Settings::load();
    ui.set_llm_model(settings.llm_model.as_str().into());
    ui.set_embeddings_model(settings.embeddings_model.as_str().into());
    ui.set_search_limit(settings.search_limit as i32);
    ui.set_classification_web(settings.classification_web);
    ui.set_sync_enabled(settings.sync_enabled);
    ui.set_sync_port(settings.sync_port.to_string().into());
    ui.set_sync_token(settings.sync_token.clone().into());
    update_pairing(&ui, &settings);
    if let Err(error) = settings.save() {
        alert(&ui, format!("Cannot save preferences: {error}"));
    }
    let status = Arc::new(std::sync::Mutex::new(sync::SyncStatus::default()));
    let server = Rc::new(RefCell::new(sync::start(settings, status.clone(), None)));
    wire_settings(&ui, status.clone(), server);
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let generation = Arc::new(AtomicU64::new(0));
    spawn_ai(&ui, receiver, generation.clone());
    wire_notes(&ui, db.clone(), sender.clone());
    wire_search(&ui, db.clone(), sender, generation);
    let view = Rc::new(RefCell::new(ViewState {
        last_id: -1,
        ..Default::default()
    }));
    wire_transfer(&ui, db.clone(), view.clone());
    refresh(&ui, &db, &mut view.borrow_mut())?;
    let handle = ui.as_weak();
    let refresh_timer = Timer::default();
    refresh_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(800),
        move || {
            if let Some(ui) = handle.upgrade() {
                if let Err(error) = refresh(&ui, &db, &mut view.borrow_mut()) {
                    alert(&ui, format!("Cannot refresh notes: {error}"));
                }
                if let Ok(state) = status.lock() {
                    ui.set_sync_running(state.running);
                    ui.set_sync_status(state.message.as_str().into());
                    ui.set_sync_devices(
                        state
                            .devices
                            .iter()
                            .map(|(_, name, last)| {
                                format!(
                                    "{}  ·  {}",
                                    name,
                                    if last.elapsed().as_secs() < 45 {
                                        "online"
                                    } else {
                                        "offline"
                                    }
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                            .into(),
                    );
                }
            }
        },
    );
    ui.run()?;
    Ok(())
}
