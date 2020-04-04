extern crate structopt;

use chrono::{Duration, NaiveDate, Weekday};
use log::{debug, error};
use phf::phf_map;
use stderrlog;
use structopt::{clap, StructOpt};

mod date;
mod process;
mod query;
use process::filter_journeys;
use query::get_trains;

static STATION_TO_ID: phf::Map<&str, i32> = phf_map! {
    "London" => 7015400,
    "Paris" => 8727100,
};

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
    verbose: usize,

    /// Since what date we should look
    #[structopt(short, long, parse(try_from_str = date::parse_date_from_str), default_value=date::NOW)]
    since: NaiveDate,

    /// To what date we should look
    #[structopt(short, long, parse(try_from_str = date::parse_date_from_str), default_value=date::PLUS_TWO_WEEKS)]
    until: NaiveDate,

    /// Number of days to stay
    #[structopt(short, long, parse(try_from_str = date::parse_duration_from_str))]
    days: Duration,

    /// Which days of the week should be considered as a start of a journey
    #[structopt(short, long, parse(try_from_str = date::parse_weekday_from_str))]
    weekday: Option<Weekday>,

    /// Max price per journey
    #[structopt(short, long)]
    price: Option<f32>,

    /// Eurostar API key
    #[structopt(short, long)]
    api_key: String,

    /// Start station
    #[structopt(parse(try_from_str = parse_station), default_value="London")]
    from: i32,

    /// Finish station
    #[structopt(parse(try_from_str = parse_station), default_value="Paris")]
    to: i32,
}

fn main() {
    let opt = Opt::from_args();
    setup_logging(opt.verbose);

    if opt.from == opt.to {
        clap::Error::value_validation_auto(
            "Start and finish stations need to be different!".to_string(),
        )
        .exit();
    }

    let travels = match date::get_possible_travel_dates(opt.since, opt.until, opt.days, opt.weekday)
    {
        Ok(res) => res,
        Err(err) => clap::Error::value_validation_auto(err.to_string()).exit(),
    };

    if travels.is_empty() {
        clap::Error::value_validation_auto(
            "There are not dates pair matching your criteria!".to_string(),
        )
        .exit();
    } else {
        debug!("Possible travel dates: {:#?}", travels);
    }

    let mut journeys = Vec::new();

    for (outbound_date, inbound_date) in travels.iter() {
        let trains = match get_trains(
            &opt.api_key,
            opt.from,
            opt.to,
            *outbound_date,
            *inbound_date,
        ) {
            Ok(res) => res,
            Err(err) => {
                error!("{:?}", err);
                std::process::exit(1);
            }
        };
        journeys.append(&mut filter_journeys(trains, opt.price));
    }

    if journeys.is_empty() {
        prinln!("There was no journey matching supplied criteria :(")
    } else {
        info!("Found {} journeys matching criteria.", journeys.len());
        println!("{:#?}", journeys);
    }
}

fn setup_logging(level: usize) {
    stderrlog::new()
        .module(module_path!())
        .verbosity(level)
        .timestamp(stderrlog::Timestamp::Off)
        .init()
        .unwrap();
}
