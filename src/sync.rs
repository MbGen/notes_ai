use crate::{db::Database, settings::Settings};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
#[path = "sync_edit.rs"]
mod edits;

#[derive(Default)]
pub struct SyncStatus {
    pub running: bool,
    pub message: String,
    pub devices: Vec<(String, String, std::time::Instant)>,
}
pub type SharedStatus = Arc<Mutex<SyncStatus>>;
#[derive(Clone)]
struct ServerState {
    token: String,
    status: SharedStatus,
    db_path: String,
}
#[derive(Deserialize)]
struct IncomingNote {
    id: String,
    text: String,
}
#[derive(Deserialize)]
struct Request {
    device_id: String,
    device_name: String,
    notes: Vec<IncomingNote>,
    #[serde(default)]
    operations: Vec<edits::Operation>,
}
#[derive(Serialize, Deserialize, Clone)]
struct Note {
    id: String,
    text: String,
    category: Option<String>,
    date: String,
    status: String,
}
#[derive(Serialize)]
struct Response {
    protocol: u32,
    accepted: Vec<String>,
    notes: Vec<Note>,
    operations: Vec<edits::Outcome>,
    categories: Vec<String>,
}

fn snapshot(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<Note>> {
    let mut notes=Vec::new();
    let mut stmt=conn.prepare("SELECT COALESCE(r.id,'pc:'||n.id),n.text,c.name,n.date FROM notes n JOIN categories c ON c.id=n.category_id LEFT JOIN sync_receipts r ON r.note_id=n.id ORDER BY n.id")?;
    for row in stmt.query_map([],|r|Ok(Note{id:r.get(0)?,text:r.get(1)?,category:Some(r.get(2)?),date:r.get(3)?,status:"classified".into()}))?{notes.push(row?);}
    let mut stmt=conn.prepare("SELECT COALESCE(r.id,'queue:'||q.id),q.text FROM notes_queue q LEFT JOIN sync_receipts r ON r.queue_id=q.id ORDER BY q.id")?;
    for row in stmt.query_map([],|r|Ok(Note{id:r.get(0)?,text:r.get(1)?,category:None,date:String::new(),status:"queued".into()}))?{notes.push(row?);}
    Ok(notes)
}

async fn sync(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> Result<Json<Response>, StatusCode> {
    if headers.get("authorization").and_then(|h| h.to_str().ok())
        != Some(format!("Bearer {}", state.token).as_str())
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if request.device_id.is_empty()
        || request.device_id.len() > 128
        || request.device_name.len() > 100
        || request.notes.len() > 200
        || request.operations.len() > 100
        || request.operations.iter().any(|op| !op.valid())
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if request.notes.iter().any(|n| {
        uuid::Uuid::parse_str(&n.id).is_err() || n.text.trim().is_empty() || n.text.len() > 100_000
    }) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let device_id = request.device_id.clone();
    let device_name = request.device_name.clone();
    let db_path = state.db_path.clone();
    let result = tokio::task::spawn_blocking(move || -> rusqlite::Result<Response> {
        let db = Database::new(&db_path);
        let conn = db.connection();
        let tx = conn.unchecked_transaction()?;
        let mut accepted = Vec::new();
        for note in request.notes {
            let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sync_receipts WHERE id=?1)", [&note.id], |r| r.get(0))?;
            if !exists {
                tx.execute("INSERT INTO notes_queue(text) VALUES (?1)", [&note.text])?;
                let queue_id = tx.last_insert_rowid();
                tx.execute("INSERT INTO sync_receipts(id,queue_id) VALUES (?1,?2)", rusqlite::params![note.id,queue_id])?;
            }
            accepted.push(note.id);
        }
        let operations=request.operations.iter().map(|op| edits::apply(&tx,op)).collect::<rusqlite::Result<Vec<_>>>()?;
        let notes=snapshot(&tx)?;
        tx.commit()?;
        let categories=db.get_categories()?.into_iter().map(|(_,name)|name).collect();
        Ok(Response {protocol:2,accepted,notes,operations,categories})
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Ok(mut status) = state.status.lock() {
        status.devices.retain(|(id, _, _)| id != &device_id);
        status
            .devices
            .push((device_id, device_name, std::time::Instant::now()));
    }
    Ok(Json(result))
}

pub fn start(
    settings: Settings,
    status: SharedStatus,
    previous: Option<tokio::task::JoinHandle<()>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Some(previous) = previous {
            previous.abort();
            let _ = previous.await;
        }
        if !settings.sync_enabled {
            if let Ok(mut s) = status.lock() {
                s.message = "Server stopped".into();
            }
            return;
        }
        // An aborted previous listener may need one scheduler turn to release its port.
        tokio::task::yield_now().await;
        let listener = match tokio::net::TcpListener::bind((
            std::net::Ipv4Addr::UNSPECIFIED,
            settings.sync_port,
        ))
        .await
        {
            Ok(listener) => listener,
            Err(error) => {
                if let Ok(mut s) = status.lock() {
                    s.message = format!("Cannot start: {error}");
                }
                return;
            }
        };
        if let Ok(mut s) = status.lock() {
            s.running = true;
            s.message = format!("Listening on LAN, port {}", settings.sync_port);
        }
        let router = Router::new()
            .route("/api/sync", post(sync))
            .layer(DefaultBodyLimit::max(2_000_000))
            .with_state(ServerState {
                token: settings.sync_token,
                status: status.clone(),
                db_path: "data/notes.db".into(),
            });
        let result = axum::serve(listener, router).await;
        if let Ok(mut s) = status.lock() {
            s.running = false;
            s.message = format!("Server stopped: {result:?}");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn operation(note:&Note,action:&str,text:&str,category:Option<&str>)->edits::Operation{
        edits::Operation{id:uuid::Uuid::new_v4().to_string(),target:note.id.clone(),action:action.into(),base:note.clone(),text:text.into(),category:category.map(str::to_string)}
    }
    async fn edit(state:ServerState,op:edits::Operation)->Response{
        let mut req=request(&uuid::Uuid::new_v4().to_string(),"unused");req.notes.clear();req.operations.push(op);
        sync(State(state),headers(),Json(req)).await.unwrap().0
    }
    #[tokio::test]
    async fn phone_edits_moves_and_deletes_are_idempotent(){
        let (path,state)=fixture();let id=uuid::Uuid::new_v4().to_string();
        let first=sync(State(state.clone()),headers(),Json(request(&id,"first text"))).await.unwrap().0;
        let op=operation(&first.notes[0],"update","edited text",None);
        let changed=edit(state.clone(),op.clone()).await;
        assert_eq!(changed.notes[0].text,"edited text");
        assert_eq!(edit(state.clone(),op).await.operations[0].outcome,"applied");
        {let db=Database::new(path.to_str().unwrap());let queue=db.get_queue_items().unwrap();db.finish_queue(queue[0].0,"Wrong stale AI","first text","today",vec![0.;768]).unwrap();assert!(db.latest_note().unwrap().is_none());}
        let moved=edit(state.clone(),operation(&changed.notes[0],"update","edited text",Some("Manual collection"))).await;
        assert_eq!(moved.notes[0].id,id);assert_eq!(moved.notes[0].category.as_deref(),Some("Manual collection"));
        {let db=Database::new(path.to_str().unwrap());let note_id=db.latest_note().unwrap().unwrap().0;db.insert_embeddings_if_text(note_id,"edited text",vec![0.;768]).unwrap();}
        let rewritten=edit(state.clone(),operation(&moved.notes[0],"update","final text",Some("Manual collection"))).await;
        {let db=Database::new(path.to_str().unwrap());let note_id=db.latest_note().unwrap().unwrap().0;
            let count=||db.connection().query_row("SELECT COUNT(*) FROM note_embeddings",[],|r|r.get::<_,i64>(0)).unwrap();
            assert_eq!(count(),0);db.insert_embeddings_if_text(note_id,"edited text",vec![0.;768]).unwrap();assert_eq!(count(),0);
            db.insert_embeddings_if_text(note_id,"final text",vec![0.;768]).unwrap();assert_eq!(count(),1);
        }
        let remove=operation(&rewritten.notes[0],"delete","",None);
        assert!(edit(state.clone(),remove.clone()).await.notes.is_empty());
        assert!(edit(state.clone(),remove).await.notes.is_empty());
        assert!(sync(State(state),headers(),Json(request(&id,"first text"))).await.unwrap().0.notes.is_empty());
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn simultaneous_pc_edit_returns_conflict_without_overwriting(){
        let (path,state)=fixture();let note;
        {let db=Database::new(path.to_str().unwrap());let cat=db.insert_category("Ideas").unwrap();let id=db.insert_note(cat,"original","2026-10-18").unwrap();note=snapshot(db.connection()).unwrap().remove(0);db.connection().execute("UPDATE notes SET text='PC edit' WHERE id=?1",[id]).unwrap();}
        let op=operation(&note,"update","phone edit",Some("Other"));
        for _ in 0..2{let response=edit(state.clone(),op.clone()).await;assert_eq!(response.operations[0].outcome,"conflict");assert_eq!(response.notes[0].text,"PC edit");}
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn server_can_start_and_restart_on_the_same_port() {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let settings = Settings { sync_enabled: true, sync_port: port, ..Settings::default() };
        let status = Arc::new(Mutex::new(SyncStatus::default()));
        let mut server = start(settings.clone(), status.clone(), None);
        for iteration in 0..2 {
            if iteration == 1 {
                status.lock().unwrap().running = false;
                server = start(settings.clone(), status.clone(), Some(server));
            }
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while !status.lock().unwrap().running {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }).await.expect("server did not start");
            let response = reqwest::get(format!("http://127.0.0.1:{port}/")).await.unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        server.abort();
        let _ = server.await;
    }
    fn fixture() -> (std::path::PathBuf, ServerState) {
        let path = std::env::temp_dir().join(format!("notes-ai-http-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(path.to_str().unwrap());
        db.init_schemas().unwrap();
        (
            path.clone(),
            ServerState {
                token: "test-key".into(),
                status: Arc::new(Mutex::new(SyncStatus::default())),
                db_path: path.to_str().unwrap().into(),
            },
        )
    }
    fn request(id: &str, text: &str) -> Request {
        Request {
            device_id: "test-phone".into(),
            device_name: "Pixel fixture".into(),
            operations: vec![],
            notes: vec![IncomingNote {
                id: id.into(),
                text: text.into(),
            }],
        }
    }
    fn headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer test-key".parse().unwrap());
        headers
    }
    #[tokio::test]
    async fn auth_and_validation_do_not_write_notes() {
        let (path, state) = fixture();
        let id = uuid::Uuid::new_v4().to_string();
        assert!(matches!(
            sync(
                State(state.clone()),
                HeaderMap::new(),
                Json(request(&id, "note"))
            )
            .await,
            Err(StatusCode::UNAUTHORIZED)
        ));
        assert!(matches!(
            sync(State(state.clone()), headers(), Json(request(&id, "  "))).await,
            Err(StatusCode::BAD_REQUEST)
        ));
        {
            let db = Database::new(path.to_str().unwrap());
            assert!(db.get_queue_items().unwrap().is_empty());
        }
        assert!(state.status.lock().unwrap().devices.is_empty());
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn retries_keep_identity_through_classification_and_do_not_resurrect_deleted_notes() {
        let (path, state) = fixture();
        let id = uuid::Uuid::new_v4().to_string();
        for _ in 0..2 {
            let response = sync(
                State(state.clone()),
                headers(),
                Json(request(&id, "offline thought")),
            )
            .await
            .unwrap()
            .0;
            assert_eq!(response.accepted, vec![id.clone()]);
            assert_eq!(response.notes.len(), 1);
            assert_eq!(response.notes[0].id, id);
            assert_eq!(response.notes[0].status, "queued");
        }
        let note_id;
        {
            let db = Database::new(path.to_str().unwrap());
            let queued = db.get_queue_items().unwrap();
            assert_eq!(queued.len(), 1);
            db.finish_queue(queued[0].0, "Ideas", &queued[0].1, "today", vec![0.0; 768])
                .unwrap();
            note_id = db.latest_note().unwrap().unwrap().0;
        }
        let response = sync(
            State(state.clone()),
            headers(),
            Json(request(&id, "offline thought")),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(response.notes.len(), 1);
        assert_eq!(response.notes[0].id, id);
        assert_eq!(response.notes[0].status, "classified");
        assert_eq!(response.notes[0].category.as_deref(), Some("Ideas"));
        {
            let db = Database::new(path.to_str().unwrap());
            db.delete_note(note_id).unwrap();
        }
        let response = sync(
            State(state),
            headers(),
            Json(request(&id, "offline thought")),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(response.accepted, vec![id]);
        assert!(response.notes.is_empty());
        std::fs::remove_file(path).unwrap();
    }
}
