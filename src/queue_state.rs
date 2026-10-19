use std::{collections::HashMap,sync::{Mutex,OnceLock}};
type State=HashMap<i64,(String,String,bool)>;
fn store()-> &'static Mutex<State> { static STORE:OnceLock<Mutex<State>>=OnceLock::new();STORE.get_or_init(||Mutex::new(HashMap::new())) }
pub fn set(id:i64,text:&str,status:&str,active:bool) { store().lock().unwrap().insert(id,(text.into(),status.into(),active)); }
pub fn rows(queue:Vec<(i64,String)>)->Vec<(i64,String,String,bool)> {
    let mut state=store().lock().unwrap();
    state.retain(|id,(text,_,_)|queue.iter().any(|(i,t)|i==id&&t==text));
    queue.into_iter().map(|(id,text)|{
        let (status,active)=state.get(&id).map(|(_,s,a)|(s.clone(),*a)).unwrap_or(("Saved · waiting for processing".into(),false));
        (id,text,status,active)
    }).collect()
}
