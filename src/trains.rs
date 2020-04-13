use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use futures::future;
use log::{debug, trace, warn};
use maplit::hashmap;
use reqwest::{Client, Error, Response};
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;

use crate::date;

#[cfg(test)]
use mockito;

static EUROSTAR_URL: &str = "https://api.prod.eurostar.com/bpa";
static SEARCH_LOCATION: &str = "train-search/uk-en";
static STATIONS_LOCATION: &str = "hotels-search/regions/uk-en";
static API_KEY_HEADER: &str = "x-apikey";

#[derive(Debug, PartialEq)]
pub struct TrainJourney {
    pub outbound: NaiveDateTime,
    pub inbound: NaiveDateTime,
    pub price: f32,
    pub out_duration: Duration,
    pub in_duration: Duration,
}

#[derive(Debug)]
pub enum QueryError {
    ReqwestError(Error),
    JsonParseError(String),
    ServerError(String),
    InternalError(String),
}

pub struct Filter {
    pub max_price: Option<f32>,
    pub out_departure_after: Option<NaiveTime>,
    pub out_departure_before: Option<NaiveTime>,
    pub in_departure_after: Option<NaiveTime>,
    pub in_departure_before: Option<NaiveTime>,
}

#[derive(Debug)]
struct Train {
    departure: NaiveDateTime,
    duration: Duration,
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
    #[serde(with = "date::naive_time", rename = "departureTime")]
    departure_time: NaiveTime,
    #[serde(with = "date::duration")]
    duration: Duration,
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

#[derive(Deserialize, Debug)]
struct Station {
    #[serde(rename = "regionName")]
    station_name: String,
    #[serde(rename = "stationId")]
    station_id: i32,
}

#[derive(Deserialize, Debug)]
struct StationsResponseJson {
    #[serde(flatten)]
    extra: HashMap<String, Station>,
}

fn filter_journeys(trains: &(Vec<Train>, Vec<Train>), filter: &Filter) -> Vec<TrainJourney> {
    let mut res = Vec::new();

    for out_t in trains.0.iter() {
        for in_t in trains.1.iter() {
            let total_price = out_t.price + in_t.price;
            if filter.max_price.is_some() && total_price > filter.max_price.unwrap() {
                continue;
            } else if filter.out_departure_after.is_some()
                && out_t.departure.time() <= filter.out_departure_after.unwrap()
            {
                continue;
            } else if filter.out_departure_before.is_some()
                && out_t.departure.time() >= filter.out_departure_before.unwrap()
            {
                continue;
            } else if filter.in_departure_after.is_some()
                && in_t.departure.time() <= filter.in_departure_after.unwrap()
            {
                continue;
            } else if filter.in_departure_before.is_some()
                && in_t.departure.time() >= filter.in_departure_before.unwrap()
            {
                continue;
            }

            res.push(TrainJourney {
                outbound: out_t.departure,
                inbound: in_t.departure,
                price: total_price,
                out_duration: out_t.duration,
                in_duration: in_t.duration,
            })
        }
    }
    res
}

pub async fn get_stations_map(api_key: &str) -> Result<HashMap<String, i32>, QueryError> {
    let client = Client::new();
    let response = do_request(&client, STATIONS_LOCATION, api_key, hashmap! {}).await?;
    let text = response.text().await.map_err(QueryError::ReqwestError)?;

    let json: StationsResponseJson = match serde_json::from_str(&text) {
        Ok(res) => res,
        Err(err) => {
            debug!("Invalid JSON: {}", text);
            return Err(QueryError::JsonParseError(format!(
                "Error while parsing JSON: {:?}",
                err
            )));
        }
    };

    let mut stations = HashMap::new();

    for (_, station) in json.extra.into_iter() {
        stations.insert(station.station_name, station.station_id);
    }

    if stations.is_empty() {
        return Err(QueryError::InternalError(
            "Server returned empty Station Name to Station ID".into(),
        ));
    }

    debug!("Got stations map: {:#?}", stations);
    Ok(stations)
}

pub async fn get_journeys(
    travels: &Vec<(NaiveDate, NaiveDate)>,
    api_key: &str,
    from: i32,
    to: i32,
    adults: i16,
    filter: &Filter,
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
        journeys.append(&mut filter_journeys(&trains?, filter));
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
    let response = do_request(
        client,
        &format!("{}/{}/{}", SEARCH_LOCATION, from, to),
        api_key,
        hashmap! {
            "outbound-date" => format_date(since),
            "inbound-date" => format_date(until),
            "adult" => adults.to_string(),
        },
    )
    .await?;

    let trains = parse_response(response, since, until).await;

    trains
}

async fn do_request(
    client: &Client,
    location: &str,
    api_key: &str,
    query_params: HashMap<&str, String>,
) -> Result<Response, QueryError> {
    #[cfg(test)]
    let url = &mockito::server_url();
    #[cfg(not(test))]
    let url = EUROSTAR_URL;

    let request = client
        .get(&format!("{}/{}", url, location))
        .query(&query_params)
        .header(API_KEY_HEADER, api_key);

    println!("Prepared request: {:?}", request);

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
    Ok(response)
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
                    duration: train.duration,
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
    use super::*;
    use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
    use mockito::{mock, Matcher, Mock};

    static API_KEY: &str = "api-key";
    static FROM: i32 = 123;
    static TO: i32 = 321;

    impl Filter {
        fn new() -> Filter {
            Filter {
                max_price: None,
                out_departure_after: None,
                out_departure_before: None,
                in_departure_before: None,
                in_departure_after: None,
            }
        }
    }
    fn create_mock() -> (Vec<(NaiveDate, NaiveDate)>, Mock) {
        let dates = vec![(
            NaiveDate::from_ymd(2020, 04, 05),
            NaiveDate::from_ymd(2020, 04, 07),
        )];
        let mock = mock(
            "GET",
            Matcher::Exact(format!("/{}/{}/{}", SEARCH_LOCATION, FROM, TO)),
        )
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("outbound-date".into(), dates[0].0.to_string().into()),
            Matcher::UrlEncoded("inbound-date".into(), dates[0].1.to_string().into()),
            Matcher::UrlEncoded("adult".into(), "2".into()),
        ]))
        .match_header("x-apikey", API_KEY)
        .with_header("content-type", "application/json");
        (dates, mock)
    }

    #[tokio::test]
    async fn test_get_journeys_filtered_by_max_price() {
        let (dates, mock) = create_mock();
        let _mock = mock
            .with_status(200)
            .with_body(include_str!("test_resources/response.json"))
            .create();
        let ref mut filter1 = Filter::new();
        filter1.max_price = Some(100.0);

        // Max price set
        let journeys = get_journeys(&dates, API_KEY, FROM, TO, 2, filter1)
            .await
            .unwrap();

        assert_eq!(
            journeys,
            vec![TrainJourney {
                outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(5, 40, 0)),
                inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(6, 33, 0)),
                price: 78.5,
                out_duration: Duration::minutes(157),
                in_duration: Duration::minutes(149),
            }]
        );
    }

    #[tokio::test]
    async fn test_get_journeys_no_filters() {
        let (dates, mock) = create_mock();
        let _mock = mock
            .with_status(200)
            .with_body(include_str!("test_resources/response.json"))
            .create();
        let ref filter = Filter::new();

        // Max price not set
        let journeys = get_journeys(&dates, API_KEY, 123, 321, 2, filter)
            .await
            .unwrap();

        assert_eq!(
            journeys,
            vec![
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(5, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(6, 33, 0)),
                    price: 78.5,
                    out_duration: Duration::minutes(157),
                    in_duration: Duration::minutes(149),
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(5, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(8, 33, 0)),
                    price: 128.5,
                    out_duration: Duration::minutes(157),
                    in_duration: Duration::minutes(149),
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(5, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(8, 53, 0)),
                    price: 128.5,
                    out_duration: Duration::minutes(157),
                    in_duration: Duration::minutes(149),
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(6, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(6, 33, 0)),
                    price: 108.5,
                    out_duration: Duration::minutes(133),
                    in_duration: Duration::minutes(149),
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(6, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(8, 33, 0)),
                    price: 158.5,
                    out_duration: Duration::minutes(133),
                    in_duration: Duration::minutes(149),
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(6, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(8, 53, 0)),
                    price: 158.5,
                    out_duration: Duration::minutes(133),
                    in_duration: Duration::minutes(149),
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(7, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(6, 33, 0)),
                    price: 128.5,
                    out_duration: Duration::minutes(133),
                    in_duration: Duration::minutes(149),
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(7, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(8, 33, 0)),
                    price: 178.5,
                    out_duration: Duration::minutes(133),
                    in_duration: Duration::minutes(149),
                },
                TrainJourney {
                    outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(7, 40, 0)),
                    inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(8, 53, 0)),
                    price: 178.5,
                    out_duration: Duration::minutes(133),
                    in_duration: Duration::minutes(149),
                },
            ]
        );
    }

    #[tokio::test]
    async fn test_get_journeys_filtered_by_time() {
        let (dates, mock) = create_mock();
        let _mock = mock
            .with_status(200)
            .with_body(include_str!("test_resources/response.json"))
            .create();
        let ref mut filter = Filter::new();
        filter.out_departure_after = Some(NaiveTime::from_hms(6, 0, 0));
        filter.out_departure_before = Some(NaiveTime::from_hms(7, 0, 0));
        filter.in_departure_after = Some(NaiveTime::from_hms(8, 0, 0));
        filter.in_departure_before = Some(NaiveTime::from_hms(8, 52, 0));

        // Departure after set
        let journeys = get_journeys(&dates, API_KEY, 123, 321, 2, filter)
            .await
            .unwrap();

        assert_eq!(
            journeys,
            vec![TrainJourney {
                outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(6, 40, 0)),
                inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(8, 33, 0)),
                price: 158.5,
                out_duration: Duration::minutes(133),
                in_duration: Duration::minutes(149),
            }]
        );
    }

    #[tokio::test]
    async fn test_get_journeys_all_filters() {
        let (dates, mock) = create_mock();
        let _mock = mock
            .with_status(200)
            .with_body(include_str!("test_resources/response.json"))
            .create();
        let ref filter = Filter {
            max_price: Some(100.0),
            out_departure_after: Some(NaiveTime::from_hms(5, 0, 0)),
            out_departure_before: Some(NaiveTime::from_hms(7, 0, 0)),
            in_departure_after: Some(NaiveTime::from_hms(6, 0, 0)),
            in_departure_before: Some(NaiveTime::from_hms(8, 30, 0)),
        };

        // Departure after set
        let journeys = get_journeys(&dates, API_KEY, 123, 321, 2, filter)
            .await
            .unwrap();

        assert_eq!(
            journeys,
            vec![TrainJourney {
                outbound: NaiveDateTime::new(dates[0].0, NaiveTime::from_hms(5, 40, 0)),
                inbound: NaiveDateTime::new(dates[0].1, NaiveTime::from_hms(6, 33, 0)),
                price: 78.5,
                out_duration: Duration::minutes(157),
                in_duration: Duration::minutes(149),
            }]
        );
    }
    #[tokio::test]
    async fn test_empty_response() {
        let (dates, mock) = create_mock();
        let _mock = mock.with_status(200).with_body("{}").create();
        let ref filter = Filter::new();

        let journeys = get_journeys(&dates, API_KEY, 123, 321, 2, filter)
            .await
            .unwrap();

        assert_eq!(journeys, vec![]);
    }

    #[tokio::test]
    async fn test_get_journeys_500_response() {
        let (dates, mock) = create_mock();
        let _mock = mock.with_status(500).with_body("server crashed").create();
        let ref filter = Filter::new();

        match get_journeys(&dates, API_KEY, FROM, TO, 2, filter).await {
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
        let ref filter = Filter::new();

        match get_journeys(&dates, API_KEY, FROM, TO, 2, filter).await {
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
        let ref filter = Filter::new();

        match get_journeys(&dates, API_KEY, FROM, TO, 2, filter).await {
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

    #[tokio::test]
    async fn test_get_stations_map_ok() {
        let _mock = mock("GET", format!("/{}", STATIONS_LOCATION).as_str())
            .with_header(API_KEY_HEADER, API_KEY)
            .with_body(include_str!("test_resources/stations.json"))
            .with_status(200)
            .create();

        let stations = get_stations_map(API_KEY).await.unwrap();

        assert_eq!(
            stations,
            hashmap! {"London".to_string() => 7015400, "Ashford".to_string() => 7054660}
        );
    }

    #[tokio::test]
    async fn test_get_stations_map_invalid_json() -> Result<(), String> {
        let _mock = mock("GET", format!("/{}", STATIONS_LOCATION).as_str())
            .with_header(API_KEY_HEADER, API_KEY)
            .with_body("{foo")
            .with_status(200)
            .create();

        match get_stations_map(API_KEY).await {
            Err(QueryError::JsonParseError(err)) => Ok(()),
            default => Err(format!(
                "get_stations_map returned: {:?} should return JsonParseError!",
                default
            )),
        }
    }

    #[tokio::test]
    async fn test_get_stations_map_empty_json() -> Result<(), String> {
        let _mock = mock("GET", format!("/{}", STATIONS_LOCATION).as_str())
            .with_header(API_KEY_HEADER, API_KEY)
            .with_body("{}")
            .with_status(200)
            .create();

        match get_stations_map(API_KEY).await {
            Err(QueryError::InternalError(err)) => Ok(()),
            default => Err(format!(
                "get_stations_map returned: {:?} should return InternalError!",
                default
            )),
        }
    }
}
