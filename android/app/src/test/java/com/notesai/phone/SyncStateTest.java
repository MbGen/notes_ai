package com.notesai.phone;
import org.json.*;
import org.junit.Test;
import static org.junit.Assert.*;

public class SyncStateTest {
    private JSONObject local(String... ids)throws JSONException{
        JSONArray out=new JSONArray();for(String id:ids)out.put(new JSONObject().put("id",id).put("text","text "+id));
        return new JSONObject().put("outbox",out).put("remote",new JSONArray());
    }
    private JSONObject response(String... ids)throws JSONException{return new JSONObject().put("accepted",new JSONArray(ids)).put("notes",new JSONArray());}
    @Test public void lateResponseRetainsNoteAddedAfterBatchWasSent()throws Exception{
        JSONArray sent=local("first").getJSONArray("outbox");
        JSONObject result=SyncState.merge(local("first","newer"),sent,response("first"));
        assertEquals(1,result.getJSONArray("outbox").length());
        assertEquals("newer",result.getJSONArray("outbox").getJSONObject(0).getString("id"));
    }
    @Test public void noAcknowledgementKeepsOfflineOutbox()throws Exception{
        JSONObject current=local("first","second");
        JSONObject result=SyncState.merge(current,current.getJSONArray("outbox"),response());
        assertEquals(2,result.getJSONArray("outbox").length());
    }
    @Test public void repeatedAcknowledgementIsIdempotent()throws Exception{
        JSONObject current=local("first");JSONArray sent=current.getJSONArray("outbox");
        JSONObject once=SyncState.merge(current,sent,response("first"));
        JSONObject twice=SyncState.merge(once,sent,response("first"));
        assertEquals(0,twice.getJSONArray("outbox").length());
    }
    @Test public void unexpectedAcknowledgementCannotDeleteUnsentNote()throws Exception{
        JSONObject current=local("first","newer");String original=current.toString();
        try{SyncState.merge(current,local("first").getJSONArray("outbox"),response("newer"));fail("Must reject unknown acknowledgement");}catch(JSONException expected){}
        assertEquals(original,current.toString());
    }
    @Test public void classificationKeepsTheRemoteIdentity()throws Exception{
        JSONObject note=new JSONObject().put("id","phone-note").put("text","an idea").put("date","today").put("category","Ideas").put("status","classified");
        JSONObject reply=response("phone-note").put("notes",new JSONArray().put(note));
        JSONObject result=SyncState.merge(local("phone-note"),local("phone-note").getJSONArray("outbox"),reply);
        assertEquals("phone-note",result.getJSONArray("remote").getJSONObject(0).getString("id"));
        assertEquals("Ideas",result.getJSONArray("remote").getJSONObject(0).getString("category"));
    }
    @Test public void invalidSnapshotDoesNotConsumeOutbox()throws Exception{
        JSONObject current=local("first");String original=current.toString();
        JSONObject reply=response("first").put("notes",new JSONArray().put(new JSONObject().put("id","bad")));
        try{SyncState.merge(current,current.getJSONArray("outbox"),reply);fail("Must reject malformed snapshot");}catch(JSONException expected){}
        assertEquals(original,current.toString());
    }
}
