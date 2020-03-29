use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use log::{debug, trace};
use reqwest::blocking::{Client, Response};
use reqwest::Error;
use serde::Deserialize;
use serde_json;

use crate::date;

static EUROSTAR_URL: &str = "https://api.prod.eurostar.com/bpa/train-search/uk-en";
static API_KEY_HEADER: &str = "x-apikey";

#[derive(Debug)]
pub enum QueryError {
    ReqwestError(Error),
    JsonParseError(String),
    InternalError(String),
}

#[derive(Debug)]
pub struct Train {
    pub departure: NaiveDateTime,
    pub price: f32,
}

#[derive(Deserialize, Debug)]
struct Price {
    #[serde(default)]
    adult: f32,
}

#[derive(Deserialize, Debug)]
struct Class {
    price: Option<Price>,
}

#[derive(Deserialize, Debug)]
struct Journey {
    #[serde(with = "date", rename = "departureTime")]
    departure_time: NaiveTime,
    class: Vec<Class>,
}

#[derive(Deserialize, Debug)]
struct InOrOut {
    journey: Vec<Journey>,
}

#[derive(Deserialize, Debug)]
struct ResponseJson {
    outbound: Option<InOrOut>,
    inbound: Option<InOrOut>,
}

pub fn get_trains(
    api_key: &str,
    from: i32,
    to: i32,
    since: NaiveDate,
    until: NaiveDate,
) -> Result<(Vec<Train>, Vec<Train>), QueryError> {
    let client = Client::new();
    let request = client
        .get(&format!("{}/{}/{}", EUROSTAR_URL, from, to))
        .query(&[
            ("outbound-date", format_date(since)),
            ("inbound-date", format_date(until)),
            ("adult", "1".to_string()),
        ])
        .header(API_KEY_HEADER, api_key);

    debug!("Prepared request: {:?}", request);

    let response = request.send().map_err(QueryError::ReqwestError)?;

    let status = response.status();

    if status.is_client_error() {
        return Err(QueryError::InternalError(format!(
            "Got {} response: {}",
            status,
            response.text().unwrap_or("".to_string()),
        )));
    } else if status.is_server_error() {
        // TODO: Retry this
        return Err(QueryError::InternalError(format!(
            "Got {} response",
            status
        )));
    } else {
        debug!("Got {} response", status);
    }

    let trains = parse_response(response, since, until);

    trains
}

fn parse_response(
    response: Response,
    out_date: NaiveDate,
    in_date: NaiveDate,
) -> Result<(Vec<Train>, Vec<Train>), QueryError> {
    let text = response.text().map_err(QueryError::ReqwestError)?;

    let json: ResponseJson = match serde_json::from_str(&text) {
        Ok(res) => res,
        Err(err) => {
            debug!("Invalid JSON: {}", text);
            return Err(QueryError::JsonParseError(format!(
                "Error while parsing JSON: {:?}",
                err
            )));
        }
    };

    let out_trains = get_trains_from_res(json.outbound, out_date);
    let in_trains = get_trains_from_res(json.inbound, in_date);

    Ok((out_trains, in_trains))
}

fn get_trains_from_res(in_or_out: Option<InOrOut>, date: NaiveDate) -> Vec<Train> {
    let mut results = Vec::new();
    if in_or_out.is_none() {
        return results;
    }

    for train in in_or_out.unwrap().journey.iter() {
        match &train.class[0].price {
            Some(val) => {
                results.push(Train {
                    price: val.adult,
                    departure: NaiveDateTime::new(date, train.departure_time),
                });
            }
            None => trace!("No value found for price in {:#?}", train),
        }
    }
    results
}
fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}