package com.notesai.phone;

import android.app.Activity;
import android.os.Bundle;
import android.content.SharedPreferences;
import android.widget.*;
import org.json.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.*;

/** Local-first phone client. All durable state mutations reload the latest stored state. */
public class MainActivity extends Activity {
    private static final Object STORE_LOCK=new Object();
    private SharedPreferences prefs;
    private ScheduledExecutorService executor;
    private volatile long generation;
    private PhoneUi phoneUi;
    private EditText host,port,token,input;
    private String deviceId;

    @Override public void onCreate(Bundle saved){
        super.onCreate(saved);
        prefs=getSharedPreferences("notes",MODE_PRIVATE);
        deviceId=prefs.getString("device",UUID.randomUUID().toString());
        boolean savedDevice=prefs.edit().putString("device",deviceId).commit();
        phoneUi=new PhoneUi(this);
        phoneUi.onManage=this::manageNote;
        host=phoneUi.host;port=phoneUi.port;token=phoneUi.token;input=phoneUi.input;
        Button connect=phoneUi.connect,scan=phoneUi.scan,add=phoneUi.add;
        host.setText(prefs.getString("host",""));port.setText(prefs.getString("port","8743"));token.setText(prefs.getString("token",""));
        if(saved!=null){host.setText(saved.getString("host",host.getText().toString()));port.setText(saved.getString("port",port.getText().toString()));token.setText(saved.getString("token",token.getText().toString()));input.setText(saved.getString("draft",""));phoneUi.selectPage(saved.getInt("page",0));phoneUi.restoreCollection(saved.getString("collection"));}
        connect.setOnClickListener(v->{
            String address=host.getText().toString().trim(),portText=port.getText().toString().trim(),key=token.getText().toString().trim();
            if(!address.matches("[A-Za-z0-9.-]+")){showStatus("Enter PC IP without http://",false);return;}
            try{int value=Integer.parseInt(portText);if(value<1024 || value>65535)throw new Exception();}catch(Exception e){showStatus("Port must be 1024–65535",false);return;}
            if(key.isEmpty()){showStatus("Enter pairing key",false);return;}
            synchronized(STORE_LOCK){if(!prefs.edit().putString("host",address).putString("port",portText).putString("token",key).commit()){showStatus("Cannot save connection",false);return;}}
            requestSync();
        });
        scan.setOnClickListener(v->com.google.mlkit.vision.codescanner.GmsBarcodeScanning.getClient(this).startScan()
            .addOnSuccessListener(barcode->{try{
                JSONObject pair=new JSONObject(barcode.getRawValue());String address=pair.getString("host"),key=pair.getString("token");int value=pair.getInt("port");
                if(!"notes-ai".equals(pair.getString("type"))||pair.getInt("version")!=1||!address.matches("[A-Za-z0-9.-]+")||value<1024||value>65535||key.isEmpty())throw new Exception();
                host.setText(address);port.setText(String.valueOf(value));token.setText(key);showStatus("Connection filled · tap Save connection & sync",false);
            }catch(Exception e){showStatus("This is not a Notes AI pairing QR",false);}})
            .addOnFailureListener(e->showStatus("Scanner unavailable · enter connection manually",false)));
        add.setOnClickListener(v->{
            String text=input.getText().toString().trim();if(text.isEmpty())return;
            if(text.getBytes(StandardCharsets.UTF_8).length>100000){showStatus("Note is too large",false);return;}
            synchronized(STORE_LOCK){try{
                JSONObject next=loadState();next.getJSONArray("outbox").put(new JSONObject().put("id",UUID.randomUUID().toString()).put("text",text).put("date",java.time.OffsetDateTime.now(java.time.ZoneOffset.UTC).toString()));
                if(!persist(next)){showStatus("Cannot save note; text kept in editor",false);return;}
            }catch(JSONException e){showStatus("Cannot read local notes. Your editor text has been kept.",false);return;}}
            input.setText("");render();requestSync();
        });
        if(!savedDevice){add.setEnabled(false);connect.setEnabled(false);showStatus("Cannot save device identity. Free phone storage and reopen the app.",false);}
        render();
    }
    @Override protected void onSaveInstanceState(Bundle out){out.putString("collection",phoneUi.currentCollection());out.putInt("page",phoneUi.currentPage());out.putString("draft",input.getText().toString());out.putString("host",host.getText().toString());out.putString("port",port.getText().toString());out.putString("token",token.getText().toString());super.onSaveInstanceState(out);}
    @Override public void onBackPressed(){if(phoneUi.currentPage()==1&&phoneUi.closeCollection())return;super.onBackPressed();}
    @Override public void onStart(){super.onStart();long run=++generation;executor=Executors.newSingleThreadScheduledExecutor();executor.scheduleWithFixedDelay(()->sync(run),0,15,TimeUnit.SECONDS);render();}
    @Override public void onStop(){generation++;if(executor!=null)executor.shutdownNow();super.onStop();}
    private void requestSync(){long run=generation;try{if(executor!=null&&!executor.isShutdown())executor.execute(()->sync(run));}catch(RejectedExecutionException ignored){}}
    private JSONObject loadState()throws JSONException{return new JSONObject(prefs.getString("state","{\"outbox\":[],\"remote\":[]}"));}
    private boolean persist(JSONObject next){return prefs.edit().putString("state",next.toString()).commit();}
    private void showStatus(String text,boolean online){runOnUiThread(()->{if(!isDestroyed())phoneUi.showStatus(text,online);});}
    private void showRunStatus(long run,String text,boolean online){runOnUiThread(()->{if(run==generation&&!isDestroyed())showStatus(text,online);});}
    private static String setting(Map<String,?> values,String name,String fallback){Object value=values.get(name);return value instanceof String?(String)value:fallback;}
    private void sync(long run){
        if(run!=generation || Thread.currentThread().isInterrupted())return;
        Map<String,?> config=prefs.getAll();String address=setting(config,"host",""),portText=setting(config,"port","8743"),key=setting(config,"token","");
        if(address.isEmpty())return;
        HttpURLConnection connection=null;
        try{
            JSONArray batch=new JSONArray(),batchOps=new JSONArray();
            synchronized(STORE_LOCK){JSONObject current=loadState();JSONArray out=current.getJSONArray("outbox");int bytes=0;java.util.Set<String> unsent=new java.util.HashSet<>(),sent=new java.util.HashSet<>();
                for(int i=0;i<out.length();i++)unsent.add(out.getJSONObject(i).getString("id"));
                for(int i=0;i<out.length()&&batch.length()<100;i++){JSONObject n=out.getJSONObject(i);int size=n.toString().getBytes(StandardCharsets.UTF_8).length;if(bytes+size>750000)break;bytes+=size;batch.put(n);sent.add(n.getString("id"));}
                JSONArray ops=SyncState.operations(current);for(int i=0;i<ops.length()&&batchOps.length()<100;i++){JSONObject op=ops.getJSONObject(i);if(unsent.contains(op.getString("target"))&&!sent.contains(op.getString("target")))continue;int size=op.toString().getBytes(StandardCharsets.UTF_8).length;if(bytes+size>1500000)break;bytes+=size;batchOps.put(op);}
            }
            JSONObject request=new JSONObject().put("device_id",deviceId).put("device_name",android.os.Build.MODEL).put("notes",batch).put("operations",batchOps);
            connection=(HttpURLConnection)new URL("http://"+address+":"+portText+"/api/sync").openConnection();connection.setConnectTimeout(5000);connection.setReadTimeout(15000);connection.setRequestMethod("POST");connection.setDoOutput(true);
            connection.setRequestProperty("Content-Type","application/json");connection.setRequestProperty("Authorization","Bearer "+key);connection.setInstanceFollowRedirects(false);
            try(java.io.OutputStream output=connection.getOutputStream()){output.write(request.toString().getBytes(StandardCharsets.UTF_8));}
            int code=connection.getResponseCode();if(code!=200)throw new Exception(code==401?"Pairing key rejected; scan the current PC QR":"PC returned HTTP "+code);
            String raw;
            try(java.io.InputStream stream=connection.getInputStream();java.io.ByteArrayOutputStream buffer=new java.io.ByteArrayOutputStream()){
                byte[] chunk=new byte[8192];int count;while((count=stream.read(chunk))!=-1){if(buffer.size()+count>20000000)throw new Exception("PC snapshot exceeds the phone size limit");buffer.write(chunk,0,count);}raw=new String(buffer.toByteArray(),StandardCharsets.UTF_8);
            }
            JSONObject response=new JSONObject(raw);
            synchronized(STORE_LOCK){
                if(run!=generation || !address.equals(prefs.getString("host","")) || !portText.equals(prefs.getString("port","8743")) || !key.equals(prefs.getString("token","")))return;
                if(!persist(SyncState.merge(loadState(),batch,batchOps,response)))throw new Exception("Cannot save sync result on phone");
            }
            runOnUiThread(()->{if(run==generation)render();});showRunStatus(run,"PC connected · last sync "+java.text.DateFormat.getTimeInstance().format(new java.util.Date()),true);
        }catch(Exception e){showRunStatus(run,"Waiting for PC · "+e.getMessage(),false);}
        finally{if(connection!=null)connection.disconnect();}
    }
    private void render(){
        synchronized(STORE_LOCK){try{phoneUi.render(SyncState.project(loadState()));}
        catch(JSONException e){showStatus("Cannot read local notes; existing storage has not been overwritten",false);}}
    }

    private void manageNote(JSONObject note,boolean delete){
        try{
            if(note.has("_conflict")){resolveConflict(note.getString("_conflict"));return;}
            if(note.optBoolean("_pending")){Toast.makeText(this,"This change is waiting for your PC",Toast.LENGTH_SHORT).show();return;}
            if(delete){new android.app.AlertDialog.Builder(this).setTitle("Delete this note?").setMessage("The deletion will sync to your PC when it is available.").setNegativeButton("Keep note",null).setPositiveButton("Delete",(dialog,which)->saveChange(note,"delete","",null)).show();return;}
            LinearLayout form=new LinearLayout(this);form.setOrientation(LinearLayout.VERTICAL);int padding=Math.round(20*getResources().getDisplayMetrics().density);form.setPadding(padding,padding,padding,padding);
            EditText text=new EditText(this);text.setText(note.getString("text"));text.setMinLines(3);text.setMaxLines(9);form.addView(text);
            AutoCompleteTextView category=new AutoCompleteTextView(this);category.setHint("Collection · leave empty for AI if queued");category.setSingleLine(true);category.setText(note.isNull("category")?"":note.optString("category"));
            java.util.Set<String> names=new java.util.TreeSet<>();synchronized(STORE_LOCK){JSONObject state=loadState();JSONArray cats=state.optJSONArray("categories");if(cats!=null)for(int i=0;i<cats.length();i++)names.add(cats.getString(i));JSONArray remote=state.getJSONArray("remote");for(int i=0;i<remote.length();i++)if(!remote.getJSONObject(i).isNull("category"))names.add(remote.getJSONObject(i).getString("category"));}
            category.setAdapter(new ArrayAdapter<>(this,android.R.layout.simple_dropdown_item_1line,names.toArray(new String[0])));category.setThreshold(0);category.setOnFocusChangeListener((v,focus)->{if(focus)category.showDropDown();});form.addView(category);
            android.app.AlertDialog dialog=new android.app.AlertDialog.Builder(this).setTitle("Edit note / move collection").setView(form).setNegativeButton("Cancel",null).setPositiveButton("Save",null).create();
            dialog.setOnShowListener(v->dialog.getButton(android.app.AlertDialog.BUTTON_POSITIVE).setOnClickListener(button->{String value=text.getText().toString().trim(),name=category.getText().toString().trim();if(value.isEmpty()||value.getBytes(StandardCharsets.UTF_8).length>100000){text.setError("Enter a note up to 100 KB");return;}if(name.getBytes(StandardCharsets.UTF_8).length>500){category.setError("Collection name is too long");return;}if(saveChange(note,"update",value,name.isEmpty()?null:name))dialog.dismiss();}));dialog.show();
        }catch(Exception e){Toast.makeText(this,"Cannot open note: "+e.getMessage(),Toast.LENGTH_LONG).show();}
    }
    private boolean saveChange(JSONObject note,String action,String text,String category){
        synchronized(STORE_LOCK){try{if(!persist(SyncState.queue(loadState(),note,action,text,category)))throw new Exception("Cannot save change on phone");}catch(Exception e){Toast.makeText(this,e.getMessage(),Toast.LENGTH_LONG).show();return false;}}
        render();requestSync();return true;
    }
    private void resolveConflict(String id)throws JSONException{
        JSONObject state; synchronized(STORE_LOCK){state=loadState();}JSONArray conflicts=state.optJSONArray("conflicts");if(conflicts==null)return;
        for(int i=0;i<conflicts.length();i++){JSONObject op=conflicts.getJSONObject(i);if(!id.equals(op.getString("id")))continue;
            String mine=op.getString("action").equals("delete")?"Delete this note":op.getString("text");
            JSONObject pc=null;JSONArray remote=state.getJSONArray("remote");for(int j=0;j<remote.length();j++)if(op.getString("target").equals(remote.getJSONObject(j).getString("id")))pc=remote.getJSONObject(j);
            String pcText=pc==null?"This note was deleted on PC.":pc.getString("text")+"\nCollection: "+pc.optString("category","Pending");
            final JSONObject displayedPc=pc;
            new android.app.AlertDialog.Builder(this).setTitle("Choose which change to keep").setMessage("Your change:\n"+mine+"\n\nCurrent PC version:\n"+pcText).setNegativeButton("Cancel",null).setNeutralButton("Keep PC",(d,w)->finishConflict(id,false,displayedPc)).setPositiveButton(pc==null?(op.getString("action").equals("delete")?"Confirm deletion":"Save my copy"):"Apply mine",(d,w)->finishConflict(id,true,displayedPc)).show();return;
        }
    }
    private void finishConflict(String id,boolean apply,JSONObject displayedPc){
        synchronized(STORE_LOCK){try{
            JSONObject state=loadState();JSONArray conflicts=state.optJSONArray("conflicts"),remaining=new JSONArray();JSONObject op=null;
            if(conflicts!=null)for(int i=0;i<conflicts.length();i++){JSONObject candidate=conflicts.getJSONObject(i);if(id.equals(candidate.getString("id")))op=candidate;else remaining.put(candidate);}
            if(op==null)return;state.put("conflicts",remaining);
            if(apply){JSONObject pc=displayedPc;
                if(pc!=null)state=SyncState.queue(state,pc,op.getString("action"),op.getString("text"),op.isNull("category")?null:op.getString("category"));
                else if(!op.getString("action").equals("delete")){JSONObject copy=new JSONObject().put("id",UUID.randomUUID().toString()).put("text",op.getString("text")).put("date",java.time.OffsetDateTime.now(java.time.ZoneOffset.UTC).toString());state.getJSONArray("outbox").put(copy);if(!op.isNull("category"))state=SyncState.queue(state,copy,"update",op.getString("text"),op.getString("category"));}
            }
            if(!persist(state))throw new Exception("Cannot save conflict choice");
        }catch(Exception e){Toast.makeText(this,e.getMessage(),Toast.LENGTH_LONG).show();return;}}
        render();requestSync();
    }
}
