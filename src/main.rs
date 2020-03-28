extern crate structopt;

use chrono::{Duration, NaiveDate, Utc};
use phf::phf_map;
use structopt::{clap, StructOpt};

static NOW: &str = "now";
static PLUS_TWO_WEEKS: &str = "+2 weeks";
static STATION_TO_ID: phf::Map<&str, i32> = phf_map! {
    "London" => 7015400,
    "Paris" => 8727100,
};

fn parse_date(date: &str) -> Result<NaiveDate, chrono::format::ParseError> {
    if date == NOW {
        return Ok(Utc::today().naive_local());
    } else if date == PLUS_TWO_WEEKS {
        return Ok(Utc::today().naive_local() + Duration::weeks(2));
    }
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
}

fn parse_station(station: &str) -> Result<i32, String> {
    match STATION_TO_ID.get(station) {
        Some(res) => Ok(*res),
        None => {
            let mut keys: Vec<&str> = STATION_TO_ID.keys().map(|&val| val).collect();
            keys.sort();
            Err(format!(
                "Invalid city name, choose from: {}.",
                keys.join(", ")
            ))
        }
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "eurostarchecker")]
struct Opt {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,

    /// Since what date we should look
    #[structopt(short, long, parse(try_from_str = parse_date), default_value=NOW)]
    since: NaiveDate,

    /// To what date we should look
    #[structopt(short, long, parse(try_from_str = parse_date), default_value=PLUS_TWO_WEEKS)]
    until: NaiveDate,

    /// Number of days to stay, if supplied it will print the return journeys
    #[structopt(short, long)]
    days: Option<i32>,

    /// Eurostar API key
    #[structopt(short, long)]
    api_key: i64,

    /// Start station
    #[structopt(parse(try_from_str = parse_station), default_value="London")]
    from: i32,

    /// Finish station
    #[structopt(parse(try_from_str = parse_station), default_value="Paris")]
    to: i32,
}

fn main() {
    let opt = Opt::from_args();

    if opt.from == opt.to {
        clap::Error::value_validation_auto(
            "Start and finish stations need to be different!".to_string(),
        )
        .exit();
    }
    println!("{:#?}", opt);
}
