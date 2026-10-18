package com.notesai.phone;

import org.json.JSONObject;
import java.time.*;
import java.time.format.DateTimeFormatter;
import java.util.*;

final class NoteOrder {
    static Long dateKey(String value) {
        value=value.trim();
        try{return OffsetDateTime.parse(value).toEpochSecond();}catch(Exception ignored){}
        for(String format:new String[]{"yyyy-MM-dd HH:mm:ss","yyyy-MM-dd'T'HH:mm:ss"}){
            try{return LocalDateTime.parse(value,DateTimeFormatter.ofPattern(format,Locale.ENGLISH)).toEpochSecond(ZoneOffset.UTC);}catch(Exception ignored){}
        }
        for(String format:new String[]{"dd MMMM yyyy","yyyy-MM-dd","dd.MM.yyyy"}){
            try{return LocalDate.parse(value,DateTimeFormatter.ofPattern(format,Locale.ENGLISH)).atStartOfDay().toEpochSecond(ZoneOffset.UTC);}catch(Exception ignored){}
        }
        return null;
    }
    static void sort(List<JSONObject> notes,boolean newest){
        notes.sort((a,b)->{
            Long x=dateKey(a.optString("date")),y=dateKey(b.optString("date"));
            if(x==null&&y!=null)return 1;if(x!=null&&y==null)return -1;
            int order=x==null?0:x.compareTo(y);
            if(order==0){try{order=Long.compare(id(a),id(b));}catch(NumberFormatException ignored){}}
            return newest?-order:order;
        });
    }
    private static long id(JSONObject note){String id=note.optString("id");return Long.parseLong(id.substring(id.lastIndexOf(':')+1));}
}
