package com.notesai.phone;

import android.app.Activity;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.Gravity;
import android.view.View;
import android.view.WindowInsets;
import android.view.inputmethod.InputMethodManager;
import android.content.Context;
import android.content.ClipboardManager;
import android.content.ClipData;
import android.view.GestureDetector;
import android.view.MotionEvent;
import android.widget.*;
import org.json.*;
import java.util.*;
import android.content.SharedPreferences;

/** Native Atelier presentation. Storage and transport remain in MainActivity. */
final class PhoneUi {
    private static final int CANVAS=Color.rgb(15,18,17), SURFACE=Color.rgb(24,29,26),
        BORDER=Color.rgb(48,55,49), INK=Color.rgb(239,236,225), MUTED=Color.rgb(171,177,164),
        GOLD=Color.rgb(211,188,141), GREEN=Color.rgb(157,193,156), RED=Color.rgb(232,163,148);
    final EditText host,port,token,input;
    final Button connect,scan,add;
    final TextView status;
    java.util.function.BiConsumer<JSONObject,Boolean> onManage;
    private final Activity activity;
    private final LinearLayout shell, pages, capture, library, connection, noteList, recent;
    private final TextView connectionBadge, libraryCount, queueCount, recentCount,changeSummary;
    private final Button[] navigation=new Button[3];
    private final EditText search;
    private JSONArray local=new JSONArray(), remote=new JSONArray();
    private JSONArray stateCategories=new JSONArray();
    private static final String PENDING="\u0000pending";
    private String category=null;
    private final SharedPreferences uiPrefs;
    private final TextView libraryTitle;
    private final Button backToClasses,sortDate;
    private boolean newest;
    private int selectedPage;

    PhoneUi(Activity activity) {
        this.activity=activity;
        uiPrefs=activity.getSharedPreferences("library-ui",Context.MODE_PRIVATE);
        newest=uiPrefs.getBoolean("newest",true);
        activity.getWindow().setStatusBarColor(CANVAS);
        activity.getWindow().setNavigationBarColor(CANVAS);
        activity.getWindow().getDecorView().setSystemUiVisibility(0);
        shell=column();shell.setBackgroundColor(CANVAS);shell.setFocusableInTouchMode(true);
        LinearLayout header=row();header.setPadding(dp(24),dp(18),dp(24),dp(14));
        TextView brand=text("NOTES AI",11,GOLD);brand.setLetterSpacing(.15f);
        header.addView(brand,new LinearLayout.LayoutParams(0,-2,1));
        connectionBadge=text("●  Offline",11,MUTED);connectionBadge.setContentDescription("Connection status: offline");
        header.addView(connectionBadge);shell.addView(header);
        divider(shell);
        pages=column();shell.addView(pages,new LinearLayout.LayoutParams(-1,0,1));
        capture=page();library=page();connection=page();

        heading(capture,"Capture.");
        LinearLayout composer=panel(capture);
        input=edit("Write a note…",false);input.setGravity(Gravity.TOP);input.setMinLines(3);
        input.setTextSize(19);input.setBackgroundColor(Color.TRANSPARENT);input.setPadding(0,dp(16),0,dp(16));
        composer.addView(input,new LinearLayout.LayoutParams(-1,-2));
        add=button("Save note  ↗",true);composer.addView(add);
        LinearLayout metrics=row();metrics.setPadding(0,dp(22),0,dp(8));
        queueCount=text("0 waiting to send",12,GOLD);metrics.addView(queueCount,new LinearLayout.LayoutParams(0,-2,1));
        recentCount=text("0 on your PC",12,MUTED);metrics.addView(recentCount);capture.addView(metrics);
        changeSummary=text("",12,GOLD);gap(capture,changeSummary,8);
        recent=column();capture.addView(recent);

        eyebrow(library,"YOUR THOUGHTS, COLLECTED");libraryTitle=serif("The library.",38);gap(library,libraryTitle,16);
        libraryCount=text("Your notes travel with you.",13,MUTED);gap(library,libraryCount,0);
        search=edit("Find a collection…",true);gap(library,search,20);
        LinearLayout controls=row();gap(library,controls,14);
        backToClasses=button("← Collections",false);controls.addView(backToClasses,new LinearLayout.LayoutParams(0,-2,1));
        sortDate=button("Date ↓ · Newest",false);LinearLayout.LayoutParams sortLp=new LinearLayout.LayoutParams(0,-2,1);sortLp.leftMargin=dp(8);controls.addView(sortDate,sortLp);
        backToClasses.setOnClickListener(v->closeCollection());
        sortDate.setOnClickListener(v->{newest=!newest;uiPrefs.edit().putBoolean("newest",newest).apply();renderLibrary();});
        noteList=column();gap(library,noteList,12);
        search.addTextChangedListener(new TextWatcher(){public void beforeTextChanged(CharSequence s,int start,int count,int after){} public void onTextChanged(CharSequence s,int start,int before,int count){renderLibrary();} public void afterTextChanged(Editable e){}});

        eyebrow(connection,"CONNECTED, ON YOUR TERMS");heading(connection,"From desk\nto pocket.");
        paragraph(connection,"A quiet connection to your own PC.");
        LinearLayout state=panel(connection);eyebrow(state,"CONNECTION");
        status=text("●  Offline · your notes are safe on this phone",13,MUTED);gap(state,status,12);
        Button retry=button("Sync now  ↗",false);gap(state,retry,14);
        LinearLayout pairing=panel(connection);eyebrow(pairing,"PAIR YOUR DEVICES");
        TextView pairHeading=serif("One scan. You’re home.",24);gap(pairing,pairHeading,12);
        TextView explanation=text("Open Preferences on your PC, enable phone sync and scan its QR code here.",13,MUTED);gap(pairing,explanation,10);
        scan=button("Scan PC QR code  ↗",true);gap(pairing,scan,18);
        eyebrow(pairing,"OR ENTER THE DETAILS");
        label(pairing,"PC address");host=edit("192.168.0.105",true);host.setInputType(android.text.InputType.TYPE_CLASS_TEXT|android.text.InputType.TYPE_TEXT_VARIATION_URI);pairing.addView(host);
        label(pairing,"Port");port=edit("8743",true);port.setInputType(android.text.InputType.TYPE_CLASS_NUMBER);pairing.addView(port);
        label(pairing,"Pairing key");token=edit("From PC preferences",true);token.setInputType(android.text.InputType.TYPE_CLASS_TEXT|android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD);pairing.addView(token);
        Switch reveal=new Switch(activity);reveal.setText(R.string.show_pairing_key);reveal.setTextColor(MUTED);reveal.setTextSize(12);gap(pairing,reveal,8);
        reveal.setOnCheckedChangeListener((button,checked)->{int position=token.getSelectionStart();token.setTransformationMethod(checked?null:android.text.method.PasswordTransformationMethod.getInstance());if(position>=0)token.setSelection(position);});
        connect=button("Save connection & sync  ↗",false);gap(pairing,connect,16);
        retry.setOnClickListener(v->connect.performClick());
        paragraph(connection,"Use the same Wi-Fi as your PC. Sync runs while this app is open. Notes stay available offline.");
        divider(shell);LinearLayout nav=row();nav.setPadding(dp(12),dp(8),dp(12),dp(8));
        String[] labels={"Capture","Library","Connect"};
        for(int i=0;i<3;i++){final int index=i;navigation[i]=button(labels[i],false);navigation[i].setTextSize(12);navigation[i].setMinHeight(dp(48));LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(0,dp(50),1);lp.setMargins(dp(3),0,dp(3),0);nav.addView(navigation[i],lp);navigation[i].setOnClickListener(v->selectPage(index));}
        shell.addView(nav);activity.setContentView(shell);
        connectionBadge.setPadding(dp(8),dp(12),0,dp(12));connectionBadge.setOnClickListener(v->selectPage(2));
        shell.setOnApplyWindowInsetsListener((view,insets)->{
            if(android.os.Build.VERSION.SDK_INT>=30){android.graphics.Insets bars=insets.getInsets(WindowInsets.Type.systemBars()|WindowInsets.Type.ime()|WindowInsets.Type.displayCutout());view.setPadding(bars.left,bars.top,bars.right,bars.bottom);}
            else view.setPadding(insets.getSystemWindowInsetLeft(),insets.getSystemWindowInsetTop(),insets.getSystemWindowInsetRight(),insets.getSystemWindowInsetBottom());
            return insets;
        });shell.requestApplyInsets();selectPage(0);
    }
    int currentPage(){return selectedPage;}
    void selectPage(int page){
        selectedPage=Math.max(0,Math.min(2,page));
        for(int i=0;i<3;i++){pages.getChildAt(i).setVisibility(i==selectedPage?View.VISIBLE:View.GONE);navigation[i].setTextColor(i==selectedPage?GOLD:MUTED);navigation[i].setBackground(shape(i==selectedPage?SURFACE:CANVAS,i==selectedPage?BORDER:CANVAS,12));navigation[i].setSelected(i==selectedPage);}
        ((InputMethodManager)activity.getSystemService(Context.INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(shell.getWindowToken(),0);
        shell.clearFocus();
    }
    void showStatus(String message,boolean online){status.setText(activity.getString(R.string.status_label,message));status.setTextColor(online?GREEN:RED);connectionBadge.setText(online?R.string.status_connected:R.string.status_disconnected);connectionBadge.setTextColor(online?GREEN:MUTED);connectionBadge.setContentDescription("Connection status: "+message);}
    void render(JSONObject state)throws JSONException{
        stateCategories=state.optJSONArray("categories");
        local=state.getJSONArray("outbox");remote=state.getJSONArray("remote");
        queueCount.setText(activity.getString(R.string.waiting_total,local.length()));recentCount.setText(activity.getString(R.string.pc_total,remote.length()));
        libraryCount.setText(activity.getString(R.string.library_total,local.length()+remote.length()));
        int changes=SyncState.operations(state).length(),conflicts=state.optJSONArray("conflicts")==null?0:state.getJSONArray("conflicts").length();
        changeSummary.setVisibility(changes>0||conflicts>0?View.VISIBLE:View.GONE);
        changeSummary.setText(changes+" pending changes"+(conflicts>0?" · "+conflicts+" conflicts":"")+(changes>0&&state.optInt("protocol",1)<2?" · Update PC app":""));
        recent.removeAllViews();
        for(int i=local.length()-1;i>=Math.max(0,local.length()-3);i--)noteCard(recent,local.getJSONObject(i),true);

        renderLibrary();
    }
    String currentCollection(){return category;}
    void restoreCollection(String name){category=name;renderLibrary();}
    boolean closeCollection(){if(category==null)return false;category=null;search.setText("");renderLibrary();return true;}
    private String visitKey(String name){return "visited:"+android.util.Base64.encodeToString(name.getBytes(java.nio.charset.StandardCharsets.UTF_8),android.util.Base64.NO_WRAP);}
    private void openCollection(String name){
        category=name;
        long order=uiPrefs.getLong("visit-counter",0)+1;
        uiPrefs.edit().putLong("visit-counter",order).putLong(visitKey(name),order).apply();
        search.setText("");renderLibrary();
        ((ScrollView)library.getParent()).smoothScrollTo(0,0);
    }
    private void renderLibrary(){
        noteList.removeAllViews();String query=search.getText().toString().trim().toLowerCase(java.util.Locale.ROOT);int count=0;
        backToClasses.setVisibility(category==null?View.GONE:View.VISIBLE);sortDate.setVisibility(category==null?View.GONE:View.VISIBLE);
        sortDate.setText(newest?"Date ↓ · Newest":"Date ↑ · Oldest");
        libraryTitle.setText(category==null?"The library.":PENDING.equals(category)?"Pending.":category);
        search.setHint(category==null?"Find a collection…":"Search this collection…");
        if(category==null){
            Map<String,Integer> totals=new HashMap<>();
            JSONArray categories=stateCategories;if(categories!=null)for(int i=0;i<categories.length();i++)totals.put(categories.optString(i),0);
            if(local.length()>0)totals.put(PENDING,local.length());
            for(int i=0;i<remote.length();i++){JSONObject n=remote.optJSONObject(i);if(n==null)continue;String name=n.isNull("category")?PENDING:n.optString("category");totals.put(name,totals.getOrDefault(name,0)+1);}
            List<String> names=new ArrayList<>(totals.keySet());
            names.sort((a,b)->{int order=Long.compare(uiPrefs.getLong(visitKey(b),0),uiPrefs.getLong(visitKey(a),0));return order!=0?order:a.compareToIgnoreCase(b);});
            for(String name:names){String title=PENDING.equals(name)?"Pending":name;if(!title.toLowerCase(Locale.ROOT).contains(query))continue;
                LinearLayout tile=panel(noteList);tile.addView(serif(title+"  ↗",25));
                gap(tile,text(totals.get(name)+" notes · "+(uiPrefs.getLong(visitKey(name),0)>0?"Recently opened":"Open collection"),12,MUTED),10);
                tile.setFocusable(true);tile.setContentDescription(title+", "+totals.get(name)+" notes. Open collection");tile.setOnClickListener(v->openCollection(name));count++;
            }
            if(count==0)empty(noteList,"A little room to begin.",query.isEmpty()?"Your collections will appear after syncing with your PC.":"No collections match this search.");
            return;
        }
        List<JSONObject> matches=new ArrayList<>();Set<String> waiting=new HashSet<>();
        for(int group=0;group<2;group++){JSONArray items=group==0?local:remote;for(int i=items.length()-1;i>=0;i--){JSONObject n=items.optJSONObject(i);if(n==null)continue;boolean pending=group==0||n.isNull("category");if(!(PENDING.equals(category)?pending:!pending&&category.equals(n.optString("category"))))continue;if(!n.optString("text").toLowerCase(Locale.ROOT).contains(query))continue;matches.add(n);if(group==0)waiting.add(n.optString("id"));}}
        if(!newest)Collections.reverse(matches);
        NoteOrder.sort(matches,newest);for(JSONObject n:matches){noteCard(noteList,n,waiting.contains(n.optString("id")));count++;}
        if(count==0)empty(noteList,"A little room to begin.",query.isEmpty()?"Save a thought in Capture, or connect to bring your PC library here.":"No notes match this search. Try another word or collection.");
    }
    private void noteCard(LinearLayout parent,JSONObject note,boolean waiting){
        LinearLayout card=panel(parent);String detail=waiting?"ON THIS PHONE · WAITING TO SEND":note.isNull("category")?"ON YOUR PC · AWAITING AI":note.optString("category");
        if(note.optBoolean("_pending"))detail="CHANGE SAVED · WAITING TO SYNC";
        if(note.has("_conflict"))detail="CONFLICT · YOUR CHANGE IS SAFE HERE";
        TextView tag=text(detail,10,waiting?GOLD:GREEN);tag.setLetterSpacing(.08f);card.addView(tag);
        TextView body=text(note.optString("text"),16,INK);body.setTextIsSelectable(true);body.setLineSpacing(dp(4),1);gap(card,body,12);
        // Observe confirmed taps while TextView retains long-press selection and scrolling.
        GestureDetector taps=new GestureDetector(activity,new GestureDetector.SimpleOnGestureListener(){
            @Override public boolean onSingleTapConfirmed(MotionEvent event){copyNote(note.optString("text"));return true;}
        });
        body.setOnTouchListener((view,event)->{taps.onTouchEvent(event);return false;});
        card.setOnClickListener(v->copyNote(note.optString("text")));
        String date=note.optString("date");if(!date.isEmpty())gap(card,text(date,10,MUTED),14);
        LinearLayout actions=row();gap(card,actions,14);
        Button edit=button(note.has("_conflict")?"Resolve conflict":"Edit / Move",false);actions.addView(edit,new LinearLayout.LayoutParams(0,-2,1));edit.setOnClickListener(v->{if(onManage!=null)onManage.accept(note,false);});
        if(!note.has("_conflict")){Button remove=button("Delete",false);remove.setTextColor(RED);LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(0,-2,1);lp.leftMargin=dp(8);actions.addView(remove,lp);remove.setOnClickListener(v->{if(onManage!=null)onManage.accept(note,true);});remove.setEnabled(!note.optBoolean("_pending"));}
        edit.setEnabled(!note.optBoolean("_pending"));
    }
    private void copyNote(String value){
        ClipboardManager clipboard=(ClipboardManager)activity.getSystemService(Context.CLIPBOARD_SERVICE);
        clipboard.setPrimaryClip(ClipData.newPlainText("Note",value));
        if(android.os.Build.VERSION.SDK_INT<33)Toast.makeText(activity,"Copied",Toast.LENGTH_SHORT).show();
    }
    private void empty(LinearLayout parent,String title,String detail){LinearLayout box=panel(parent);box.addView(serif(title,23));gap(box,text(detail,12,MUTED),10);}
    private LinearLayout page(){ScrollView scroll=new ScrollView(activity);scroll.setFillViewport(true);scroll.setClipToPadding(false);LinearLayout content=column();content.setPadding(dp(24),dp(26),dp(24),dp(28));scroll.addView(content);pages.addView(scroll,new LinearLayout.LayoutParams(-1,-1));return content;}
    private LinearLayout panel(LinearLayout parent){LinearLayout box=column();box.setPadding(dp(20),dp(20),dp(20),dp(20));box.setBackground(shape(SURFACE,BORDER,18));gap(parent,box,16);return box;}
    private void eyebrow(LinearLayout parent,String label){TextView view=text(label,10,GOLD);view.setLetterSpacing(.17f);gap(parent,view,16);}
    private void heading(LinearLayout parent,String title){TextView view=serif(title,38);view.setLineSpacing(0,1.03f);gap(parent,view,16);}
    private void paragraph(LinearLayout parent,String value){TextView view=text(value,13,MUTED);view.setLineSpacing(dp(4),1);gap(parent,view,14);}
    private void label(LinearLayout parent,String value){gap(parent,text(value,12,MUTED),18);}
    private EditText edit(String hint,boolean single){EditText e=new EditText(activity);e.setHint(hint);e.setTextColor(INK);e.setHintTextColor(MUTED);e.setTextSize(15);e.setSingleLine(single);e.setPadding(dp(14),dp(14),dp(14),dp(14));e.setMinHeight(dp(52));e.setBackground(shape(CANVAS,BORDER,10));e.setTypeface(Typeface.create("sans-serif",Typeface.NORMAL));e.setBackgroundTintList(null);e.setSelectAllOnFocus(false);return e;}
    private Button button(String title,boolean primary){Button b=new Button(activity);b.setText(title);b.setAllCaps(false);b.setTextSize(14);b.setTypeface(Typeface.create("sans-serif-medium",Typeface.NORMAL));b.setTextColor(primary?CANVAS:INK);b.setMinHeight(dp(52));b.setMinimumWidth(0);b.setPadding(dp(16),dp(8),dp(16),dp(8));android.graphics.drawable.StateListDrawable states=new android.graphics.drawable.StateListDrawable();states.addState(new int[]{android.R.attr.state_pressed},shape(primary?Color.rgb(185,161,118):BORDER,BORDER,10));states.addState(new int[]{},shape(primary?GOLD:SURFACE,primary?GOLD:BORDER,10));b.setBackground(states);b.setBackgroundTintList(null);return b;}
    private TextView text(String value,int size,int color){TextView t=new TextView(activity);t.setText(value);t.setTextSize(size);t.setTextColor(color);t.setFontFeatureSettings("kern");t.setTypeface(Typeface.create("sans-serif",Typeface.NORMAL));return t;}
    private TextView serif(String value,int size){TextView t=text(value,size,INK);t.setTypeface(Typeface.create("serif",Typeface.NORMAL));return t;}
    private LinearLayout column(){LinearLayout view=new LinearLayout(activity);view.setOrientation(LinearLayout.VERTICAL);return view;}
    private LinearLayout row(){LinearLayout view=new LinearLayout(activity);view.setOrientation(LinearLayout.HORIZONTAL);view.setGravity(Gravity.CENTER_VERTICAL);return view;}
    private void gap(LinearLayout parent,View child,int top){LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(top);parent.addView(child,lp);}
    private void divider(LinearLayout parent){View line=new View(activity);line.setBackgroundColor(BORDER);parent.addView(line,new LinearLayout.LayoutParams(-1,dp(1)));}
    private GradientDrawable shape(int fill,int stroke,int radius){GradientDrawable d=new GradientDrawable();d.setColor(fill);d.setCornerRadius(dp(radius));d.setStroke(dp(1),stroke);return d;}
    private int dp(int value){return Math.round(value*activity.getResources().getDisplayMetrics().density);}
}
