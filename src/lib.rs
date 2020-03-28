use chrono::{NaiveDate, Utc};

#[derive(Debug)]
pub struct Train {
    from: i32,
    to: i32,
    departure: NaiveDate,
    price: i16,
}

pub fn get_trains(from: i32, to: i32, since: NaiveDate, until: NaiveDate, price: Option<i16>) -> Vec<Train> {
    vec![Train{from: from, to: to, departure: Utc::today().naive_local(), price: 100}]
}
