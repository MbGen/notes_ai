package com.notesai.phone;
import org.junit.Test;
import org.json.JSONObject;
import java.util.*;
import static org.junit.Assert.*;

public class NoteOrderTest {
    private JSONObject note(String id,String date)throws Exception{return new JSONObject().put("id",id).put("date",date);}
    @Test public void datesOverrideIdsAndUnknownDatesStayLast()throws Exception{
        List<JSONObject> notes=new ArrayList<>(Arrays.asList(note("pc:90","01 January 2024"),note("pc:1","2026-10-18"),note("pc:100","unknown"),note("pc:2","18.10.2026")));
        NoteOrder.sort(notes,true);assertEquals("pc:2",notes.get(0).getString("id"));assertEquals("pc:90",notes.get(2).getString("id"));assertEquals("pc:100",notes.get(3).getString("id"));
        NoteOrder.sort(notes,false);assertEquals("pc:90",notes.get(0).getString("id"));assertEquals("pc:1",notes.get(1).getString("id"));assertEquals("pc:100",notes.get(3).getString("id"));
    }
    @Test public void timestampsCompareChronologicallyAcrossOffsets(){assertEquals(NoteOrder.dateKey("2026-10-18T12:00:00+03:00"),NoteOrder.dateKey("2026-10-18T09:00:00Z"));}
}
