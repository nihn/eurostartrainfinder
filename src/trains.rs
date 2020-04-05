use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use futures::future;
use log::{debug, trace, warn};
use reqwest::{Client, Error, Response};
use serde::Deserialize;
use serde_json;

use crate::date;

#[cfg(test)]
use mockito;

static EUROSTAR_URL: &str = "https://api.prod.eurostar.com/bpa/train-search/uk-en";
static API_KEY_HEADER: &str = "x-apikey";

#[derive(Debug, PartialEq)]
pub struct TrainJourney {
    pub outbound: NaiveDateTime,
    pub inbound: NaiveDateTime,
    pub price: f32,
}

#[derive(Debug)]
pub enum QueryError {
    ReqwestError(Error),
    JsonParseError(String),
    ServerError(String),
    InternalError(String),
}

#[derive(Debug)]
struct Train {
    departure: NaiveDateTime,
    price: f32,
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

fn filter_journeys(trains: &(Vec<Train>, Vec<Train>), max_price: Option<f32>) -> Vec<TrainJourney> {
    let mut res = Vec::new();

    for out_t in trains.0.iter() {
        for in_t in trains.1.iter() {
            let total_price = out_t.price + in_t.price;
            if max_price.is_some() && total_price > max_price.unwrap() {
                continue;
            }
            res.push(TrainJourney {
                outbound: out_t.departure,
                inbound: in_t.departure,
                price: total_price,
            })
        }
    }
    res
}

pub async fn get_journeys(
    travels: &Vec<(NaiveDate, NaiveDate)>,
    api_key: &str,
    from: i32,
    to: i32,
    adults: i16,
    max_price: Option<f32>,
) -> Result<Vec<TrainJourney>, QueryError> {
    let client = Client::new();
    let mut all_trains = Vec::new();

    for (outbound_date, inbound_date) in travels.iter() {
        all_trains.push(get_trains(
            &client,
            api_key,
            from,
            to,
            *outbound_date,
            *inbound_date,
            adults,
        ));
    }

    let mut journeys = Vec::new();

    for trains in future::join_all(all_trains).await {
        journeys.append(&mut filter_journeys(&trains?, max_price));
    }
    Ok(journeys)
}

async fn get_trains(
    client: &Client,
    api_key: &str,
    from: i32,
    to: i32,
    since: NaiveDate,
    until: NaiveDate,
    adults: i16,
) -> Result<(Vec<Train>, Vec<Train>), QueryError> {
    #[cfg(test)]
    let url = &mockito::server_url();
    #[cfg(not(test))]
    let url = EUROSTAR_URL;

    let request = client
        .get(&format!("{}/{}/{}", url, from, to))
        .query(&[
            ("outbound-date", format_date(since)),
            ("inbound-date", format_date(until)),
            ("adult", adults.to_string()),
        ])
        .header(API_KEY_HEADER, api_key);

    debug!("Prepared request: {:?}", request);

    let response = request.send().await.map_err(QueryError::ReqwestError)?;

    let status = response.status();

    if status.is_client_error() {
        return Err(QueryError::InternalError(format!(
            "Got {} response: {}",
            status,
            response.text().await.unwrap_or("".to_string()),
        )));
    } else if status.is_server_error() {
        // TODO: Retry this
        return Err(QueryError::ServerError(format!(
            "Got {} response: {}",
            status,
            response.text().await.unwrap_or("".to_string()),
        )));
    } else {
        debug!("Got {} response", status);
    }

    let trains = parse_response(response, since, until).await;

    trains
}

async fn parse_response(
    response: Response,
    out_date: NaiveDate,
    in_date: NaiveDate,
) -> Result<(Vec<Train>, Vec<Train>), QueryError> {
    let text = response.text().await.map_err(QueryError::ReqwestError)?;

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

    if json.outbound.is_none() || json.inbound.is_none() {
        warn!("No trains found for {} and {} date pair", out_date, in_date);
    }

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

#[cfg(test)]
mod tests {
    use crate::trains::{get_journeys, QueryError, TrainJourney};
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use mockito::{mock, Mock};

    static API_KEY: &str = "api-key";
    static FROM: i32 = 123;
    static TO: i32 = 321;

    fn create_mock() -> (Vec<(NaiveDate, NaiveDate)>, Mock) {
        let dates = vec![(
            NaiveDate::from_ymd(2020, 04, 05),
            NaiveDate::from_ymd(2020, 04, 07),
        )];

        let mock = mock(
            "GET",
            format!(
                "/{}/{}?outbound-date={}&inbound-date={}&adult=2",
                FROM, TO, dates[0].0, dates[0].1
            )
            .as_str(),
        )
        .with_header("content-type", "application/json")
        .match_header("x-apikey", API_KEY);

        (dates, mock)
    }

    #[tokio::test]
    async fn test_get_journeys_ok() {
        let (dates, mut mock) = create_mock();
        mock = mock
            .with_status(200)
            .with_body(include_str!("test_resources/response.json"))
            .create();

        // Max price set
        let journeys = get_journeys(&dates, API_KEY, FROM, TO, 2, Some(100.0))
            .await
            .unwrap();

        assert_eq!(
            journeys,
            vec![TrainJourney {
                outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(5, 40, 0)),
                inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(6, 33, 0)),
                price: 78.5,
            }]
        );

        mock.create();

        // Max price not set
        let journeys = get_journeys(&dates, "api-key", 123, 321, 2, None)
            .await
            .unwrap();

        assert_eq!(
            journeys,
            vec![
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(5, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(6, 33, 0)),
                    price: 78.5,
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(6, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(6, 33, 0)),
                    price: 108.5,
                }
            ]
        );
    }

    #[tokio::test]
    async fn test_empty_response() {
        let (dates, mock) = create_mock();
        let _mock = mock.with_status(200).with_body("{}").create();

        let journeys = get_journeys(&dates, "api-key", 123, 321, 2, None)
            .await
            .unwrap();

        assert_eq!(journeys, vec![]);
    }

    #[tokio::test]
    async fn test_get_journeys_500_response() {
        let (dates, mock) = create_mock();
        let _mock = mock.with_status(500).with_body("server crashed").create();

        match get_journeys(&dates, API_KEY, FROM, TO, 2, Some(100.0)).await {
            Err(QueryError::ServerError(err)) => assert_eq!(
                err,
                "Got 500 Internal Server Error response: server crashed"
            ),
            default => panic!(
                "get_journeys return {:?}, it should return QueryError::ServerError!",
                default
            ),
        }
    }

    #[tokio::test]
    async fn test_get_journeys_400_response() {
        let (dates, mock) = create_mock();
        let _mock = mock.with_status(404).with_body("never existed").create();

        match get_journeys(&dates, API_KEY, FROM, TO, 2, Some(100.0)).await {
            Err(QueryError::InternalError(err)) => {
                assert_eq!(err, "Got 404 Not Found response: never existed")
            }
            default => panic!(
                "get_journeys return {:?}, it should return QueryError::InternalError!",
                default
            ),
        }
    }

    #[tokio::test]
    async fn test_get_journeys_invalid_json() {
        let (dates, mock) = create_mock();
        let _mock = mock.with_status(200).with_body("not a json").create();

        match get_journeys(&dates, API_KEY, FROM, TO, 2, Some(100.0)).await {
            Err(QueryError::JsonParseError(err)) => assert_eq!(
                err,
                "Error while parsing JSON: Error(\"expected ident\", line: 1, column: 2)"
            ),
            default => panic!(
                "get_journeys returned {:?}, it should return QueryError::JsonParseError!",
                default
            ),
        }
    }
}
