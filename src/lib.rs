use chrono::{NaiveDate, Utc};
use reqwest::{Client, RequestBuilder};

static EUROSTAR_URL: &str = "https://api.prod.eurostar.com/bpa/train-search/uk-en/";
static API_KEY_HEADER: &str = "x-apikey";


#[derive(Debug)]
pub struct Train {
    from: i32,
    to: i32,
    departure: NaiveDate,
    price: i16,
}

pub fn get_trains(
    api_key: i64,
    from: i32,
    to: i32,
    since: NaiveDate,
    until: NaiveDate,
    price: Option<i16>,
) -> Vec<Train> {
    let client = Client::new();
    let request = client
        .get(format!("{}/{}/{}", EUROSTAR_URL, from, to).as_str())
        .query(&[
            ("outbound-date", format_date(since)),
            ("inbound-date", format_date(until)),
            ("adult", "1".to_string()),
        ])
        .header(API_KEY_HEADER, api_key);

    vec![Train {
        from: from,
        to: to,
        departure: Utc::today().naive_local(),
        price: 100,
    }]
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}