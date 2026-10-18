package com.notesai.phone;
import org.json.*;
import org.junit.Test;
import static org.junit.Assert.*;

public class NoteEditsTest {
    private JSONObject note()throws Exception{return new JSONObject().put("id","pc:1").put("text","original").put("category","Ideas").put("date","2026-10-18").put("status","classified");}
    private JSONObject state()throws Exception{return new JSONObject().put("outbox",new JSONArray()).put("remote",new JSONArray().put(note()));}
    private JSONObject response(JSONArray notes,JSONArray outcomes)throws Exception{return new JSONObject().put("protocol",2).put("accepted",new JSONArray()).put("notes",notes).put("operations",outcomes);}
    @Test public void editSavedWhileUploadWasInFlightSurvivesOldSnapshot()throws Exception{
        JSONObject current=SyncState.queue(state(),note(),"update","offline edit","Other");
        JSONObject next=SyncState.merge(current,new JSONArray(),new JSONArray(),response(state().getJSONArray("remote"),new JSONArray()));
        assertEquals(1,SyncState.operations(next).length());
        JSONObject visible=SyncState.project(next).getJSONArray("remote").getJSONObject(0);
        assertEquals("offline edit",visible.getString("text"));assertEquals("Other",visible.getString("category"));
    }
    @Test public void conflictKeepsLocalIntentAndPcVersion()throws Exception{
        JSONObject current=SyncState.queue(state(),note(),"update","my edit","Other");JSONArray sent=SyncState.operations(current);
        JSONObject pc=note().put("text","PC edit");JSONArray outcomes=new JSONArray().put(new JSONObject().put("id",sent.getJSONObject(0).getString("id")).put("outcome","conflict"));
        JSONObject next=SyncState.merge(current,new JSONArray(),sent,response(new JSONArray().put(pc),outcomes));
        assertEquals(0,SyncState.operations(next).length());assertEquals("my edit",next.getJSONArray("conflicts").getJSONObject(0).getString("text"));assertEquals("PC edit",next.getJSONArray("remote").getJSONObject(0).getString("text"));
        assertEquals(2,SyncState.project(next).getJSONArray("remote").length());
    }
    @Test public void offlineDeleteStaysHiddenOnOldPcButIsNotDiscarded()throws Exception{
        JSONObject current=SyncState.queue(state(),note(),"delete","",null);
        JSONObject oldPc=new JSONObject().put("accepted",new JSONArray()).put("notes",state().getJSONArray("remote"));
        JSONObject next=SyncState.merge(current,new JSONArray(),SyncState.operations(current),oldPc);
        assertEquals(1,SyncState.operations(next).length());assertEquals(0,SyncState.project(next).getJSONArray("remote").length());assertEquals(1,next.getInt("protocol"));
    }
    @Test public void unknownOperationAckCannotConsumePendingChange()throws Exception{
        JSONObject current=SyncState.queue(state(),note(),"update","mine",null);
        JSONArray wrong=new JSONArray().put(new JSONObject().put("id","not-sent").put("outcome","applied"));
        try{SyncState.merge(current,new JSONArray(),SyncState.operations(current),response(state().getJSONArray("remote"),wrong));fail();}catch(JSONException expected){}
        assertEquals(1,SyncState.operations(current).length());
    }
    @Test public void localCreateThenMoveUsesQueuedBaseAndKeepsRawUpload()throws Exception{
        JSONObject local=new JSONObject().put("id","test-local").put("text","new thought").put("date","2026-10-18T09:00:00Z");
        JSONObject current=state().put("outbox",new JSONArray().put(local));
        JSONObject next=SyncState.queue(current,local,"update","changed thought","Tools");
        assertEquals("new thought",next.getJSONArray("outbox").getJSONObject(0).getString("text"));
        JSONObject base=SyncState.operations(next).getJSONObject(0).getJSONObject("base");assertEquals("queued",base.getString("status"));assertEquals("",base.getString("date"));assertTrue(base.isNull("category"));
    }
}
