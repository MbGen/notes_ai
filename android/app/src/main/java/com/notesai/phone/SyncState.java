package com.notesai.phone;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import java.util.HashSet;

/** Pure state transition: ACKs apply only to the sent batch, never to newer local notes. */
final class SyncState {
    static JSONObject merge(JSONObject current, JSONArray sent, JSONObject response) throws JSONException {
        return merge(current,sent,new JSONArray(),response);
    }
    static JSONObject merge(JSONObject current, JSONArray sent, JSONArray sentOps, JSONObject response) throws JSONException {
        HashSet<String> submitted = new HashSet<>();
        for (int i=0;i<sent.length();i++) submitted.add(sent.getJSONObject(i).getString("id"));
        HashSet<String> accepted = new HashSet<>();
        JSONArray acknowledgements=response.getJSONArray("accepted");
        for(int i=0;i<acknowledgements.length();i++){
            String id=acknowledgements.getString(i);
            if(!submitted.contains(id)) throw new JSONException("PC acknowledged a note outside the submitted batch");
            accepted.add(id);
        }
        JSONArray remote=response.getJSONArray("notes");
        HashSet<String> remoteIds=new HashSet<>();
        for(int i=0;i<remote.length();i++){
            JSONObject note=remote.getJSONObject(i);
            String id=note.getString("id"),status=note.getString("status");
            if(id.isEmpty() || !remoteIds.add(id) || !(status.equals("queued") || status.equals("classified"))) throw new JSONException("Invalid PC snapshot");
            note.getString("text");note.getString("date");
            if(!note.isNull("category"))note.getString("category");
        }
        JSONArray remaining=new JSONArray(),outbox=current.getJSONArray("outbox");
        for(int i=0;i<outbox.length();i++){
            JSONObject note=outbox.getJSONObject(i);
            if(!accepted.contains(note.getString("id")))remaining.put(note);
        }
        JSONObject next=new JSONObject(current.toString());
        JSONArray pending=operations(current),conflicts=next.optJSONArray("conflicts");if(conflicts==null)conflicts=new JSONArray();
        HashSet<String> submittedOps=new HashSet<>(),finished=new HashSet<>();
        for(int i=0;i<sentOps.length();i++)submittedOps.add(sentOps.getJSONObject(i).getString("id"));
        JSONArray results=response.optJSONArray("operations");
        if(results!=null)for(int i=0;i<results.length();i++){
            JSONObject result=results.getJSONObject(i);String id=result.getString("id"),outcome=result.getString("outcome");
            if(!submittedOps.contains(id)||!finished.add(id)||!(outcome.equals("applied")||outcome.equals("conflict")||outcome.equals("missing")))throw new JSONException("Invalid operation acknowledgement");
            if(!outcome.equals("applied"))for(int j=0;j<pending.length();j++){JSONObject op=pending.getJSONObject(j);if(op.getString("id").equals(id)){JSONObject conflict=new JSONObject(op.toString());conflict.put("reason",outcome);conflicts.put(conflict);}}
        }
        JSONArray rest=new JSONArray();for(int i=0;i<pending.length();i++)if(!finished.contains(pending.getJSONObject(i).getString("id")))rest.put(pending.getJSONObject(i));
        return next.put("outbox",remaining).put("remote",new JSONArray(remote.toString())).put("operations",rest).put("conflicts",conflicts).put("protocol",response.optInt("protocol",1)).put("categories",response.optJSONArray("categories")==null?new JSONArray():response.getJSONArray("categories"));
    }
    static JSONArray operations(JSONObject current){JSONArray ops=current.optJSONArray("operations");return ops==null?new JSONArray():ops;}
    static JSONObject queue(JSONObject current,JSONObject note,String action,String text,String category)throws JSONException{
        JSONObject next=new JSONObject(current.toString());JSONArray ops=operations(next);String target=note.getString("id");
        for(int i=0;i<ops.length();i++)if(ops.getJSONObject(i).getString("target").equals(target))throw new JSONException("This note already has a change waiting to sync");
        JSONObject base=new JSONObject(note.toString());base.put("status",note.optString("status","queued"));
        if(!note.has("status")){base.put("category",JSONObject.NULL);base.put("date","");}
        JSONObject op=new JSONObject().put("id",java.util.UUID.randomUUID().toString()).put("target",target).put("action",action).put("base",base).put("text",text).put("category",category==null?JSONObject.NULL:category);
        ops.put(op);return next.put("operations",ops);
    }
    static JSONObject project(JSONObject current)throws JSONException{
        JSONObject next=new JSONObject(current.toString());JSONArray ops=operations(current);
        for(String key:new String[]{"outbox","remote"}){
            JSONArray source=next.getJSONArray(key),visible=new JSONArray();
            for(int i=0;i<source.length();i++){
                JSONObject note=source.getJSONObject(i);boolean deleted=false;
                for(int j=0;j<ops.length();j++){JSONObject op=ops.getJSONObject(j);if(!op.getString("target").equals(note.getString("id")))continue;
                    deleted=op.getString("action").equals("delete");note.put("_pending",true);
                    if(!deleted){note.put("text",op.getString("text"));if(!op.isNull("category"))note.put("category",op.getString("category"));}
                }
                if(!deleted)visible.put(note);
            }next.put(key,visible);
        }
        JSONArray conflicts=current.optJSONArray("conflicts");if(conflicts!=null)for(int i=0;i<conflicts.length();i++){
            JSONObject op=conflicts.getJSONObject(i),note=new JSONObject(op.getJSONObject("base").toString());
            note.put("_conflict",op.getString("id")).put("text",op.getString("action").equals("delete")?op.getJSONObject("base").getString("text"):op.getString("text"));
            if(!op.isNull("category"))note.put("category",op.getString("category"));next.getJSONArray("remote").put(note);
        }
        return next;
    }
}
