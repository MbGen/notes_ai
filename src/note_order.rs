use chrono::{DateTime, NaiveDate, NaiveDateTime};

fn date_key(value: &str) -> Option<i64> {
    let value = value.trim();
    if let Ok(date) = DateTime::parse_from_rfc3339(value) { return Some(date.timestamp()); }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(date) = NaiveDateTime::parse_from_str(value, format) { return Some(date.and_utc().timestamp()); }
    }
    for format in ["%d %B %Y", "%Y-%m-%d", "%d.%m.%Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(value, format) { return Some(date.and_hms_opt(0,0,0).unwrap().and_utc().timestamp()); }
    }
    None
}

pub fn sort(notes: &mut [(i64, String, String)], newest: bool) {
    notes.sort_by(|a,b| {
        match (date_key(&a.2),date_key(&b.2)) {
            (Some(x),Some(y)) => if newest { y.cmp(&x) } else { x.cmp(&y) },
            (Some(_),None) => std::cmp::Ordering::Less,
            (None,Some(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }.then_with(|| if newest { b.0.cmp(&a.0) } else { a.0.cmp(&b.0) })
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates_override_ids_and_unknown_dates_stay_last() {
        let mut notes=vec![(90,"".into(),"01 January 2024".into()),(1,"".into(),"2026-10-18".into()),(100,"".into(),"unknown".into()),(2,"".into(),"18.10.2026".into())];
        super::sort(&mut notes,true);
        assert_eq!(notes.iter().map(|n|n.0).collect::<Vec<_>>(),vec![2,1,90,100]);
        super::sort(&mut notes,false);
        assert_eq!(notes.iter().map(|n|n.0).collect::<Vec<_>>(),vec![90,1,2,100]);
    }
}
