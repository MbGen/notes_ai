use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize,Serialize};

#[derive(Deserialize,Serialize,Clone)]
pub struct Operation {
    pub id:String,
    pub target:String,
    pub action:String,
    pub base:super::Note,
    #[serde(default)] pub text:String,
    pub category:Option<String>,
}
#[derive(Serialize)]
pub struct Outcome { pub id:String, pub outcome:String }
impl Operation {
    pub fn valid(&self)->bool {
        uuid::Uuid::parse_str(&self.id).is_ok() && self.target.len()<=128 && self.base.id==self.target
        && self.base.text.len()<=100_000 && self.base.date.len()<=128
        && self.base.category.as_ref().map_or(true,|c|c.len()<=500)
        && (self.action=="delete" || (self.action=="update" && !self.text.trim().is_empty() && self.text.len()<=100_000
            && self.category.as_ref().map_or(true,|c|!c.trim().is_empty()&&c.len()<=500)))
    }
}
pub fn apply(tx:&Transaction<'_>,op:&Operation)->rusqlite::Result<Outcome>{
    if let Some(outcome)=tx.query_row("SELECT outcome FROM sync_operations WHERE id=?1",[&op.id],|r|r.get::<_,String>(0)).optional()?{
        return Ok(Outcome{id:op.id.clone(),outcome});
    }
    let current=super::snapshot(tx)?.into_iter().find(|n|n.id==op.target);
    let outcome=match current {
        None if op.action=="delete"=>"applied",
        None=>"missing",
        Some(ref n) if n.text!=op.base.text || n.category!=op.base.category || n.date!=op.base.date || n.status!=op.base.status=>"conflict",
        Some(n)=>{
            let (table,column,prefix)=if n.status=="queued"{("notes_queue","queue_id","queue:")}else{("notes","note_id","pc:")};
            let id=if let Some(id)=op.target.strip_prefix(prefix){id.parse::<i64>().map_err(|_|rusqlite::Error::InvalidQuery)?}
                else{tx.query_row(&format!("SELECT {column} FROM sync_receipts WHERE id=?1"),[&op.target],|r|r.get::<_,i64>(0))?};
            if op.action=="delete"{
                if table=="notes"{tx.execute("DELETE FROM note_embeddings WHERE note_id=?1",[id])?;}
                tx.execute(&format!("DELETE FROM {table} WHERE id=?1"),[id])?;
            }else if table=="notes_queue" && op.category.is_none(){
                tx.execute("UPDATE notes_queue SET text=?1 WHERE id=?2",params![op.text.trim(),id])?;
            }else{
                let name=op.category.as_deref().or(n.category.as_deref()).unwrap_or("Uncategorized").trim();
                tx.execute("INSERT INTO categories(name) SELECT ?1 WHERE NOT EXISTS(SELECT 1 FROM categories WHERE lower(name)=lower(?1))",[name])?;
                let category:i64=tx.query_row("SELECT id FROM categories WHERE lower(name)=lower(?1) LIMIT 1",[name],|r|r.get(0))?;
                if table=="notes"{
                    tx.execute("UPDATE notes SET text=?1,category_id=?2 WHERE id=?3",params![op.text.trim(),category,id])?;
                    if n.text!=op.text.trim(){tx.execute("DELETE FROM note_embeddings WHERE note_id=?1",[id])?;}
                }else{
                    // Keep the wire identity when a queued note is manually filed.
                    tx.execute("INSERT OR IGNORE INTO sync_receipts(id,queue_id) VALUES (?1,?2)",params![op.target,id])?;
                    let date=chrono::Local::now().format("%d %B %Y").to_string();
                    tx.execute("INSERT INTO notes(text,category_id,date) VALUES (?1,?2,?3)",params![op.text.trim(),category,date])?;
                    let note_id=tx.last_insert_rowid();
                    tx.execute("UPDATE sync_receipts SET queue_id=NULL,note_id=?1 WHERE queue_id=?2",params![note_id,id])?;
                    tx.execute("DELETE FROM notes_queue WHERE id=?1",[id])?;
                }
            }
            "applied"
        }
    }.to_string();
    tx.execute("INSERT INTO sync_operations(id,outcome) VALUES (?1,?2)",params![op.id,outcome])?;
    Ok(Outcome{id:op.id.clone(),outcome})
}
