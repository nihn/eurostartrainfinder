extern crate structopt;

use chrono::{Duration, NaiveDate, NaiveTime, Weekday};
use clap::arg_enum;
use log::{debug, error, info};
use prettytable::{cell, format, row, Table};
use std::collections::HashMap;
use stderrlog;
use structopt::{clap, StructOpt};
use tokio;
mod date;
mod trains;
use trains::{get_journeys, get_stations_map, Filter, TrainJourney};

static RESULT_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M";

arg_enum! {
    #[derive(Debug)]
    enum SortBy {
        Price,
        Date,
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "eurostarchecker")]
struct Opt {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: usize,

    /// Since what date we should look
    #[structopt(short, long, value_name = "YYYY-MM-DD", parse(try_from_str = date::parse_date_from_str), default_value=date::NOW)]
    since: NaiveDate,

    /// To what date we should look
    #[structopt(short, long, value_name = "YYYY-MM-DD", parse(try_from_str = date::parse_date_from_str), default_value=date::PLUS_TWO_WEEKS)]
    until: NaiveDate,

    /// Number of days to stay (e.g. Friday - Sunday would be 3 days)
    #[structopt(short, long, parse(try_from_str = date::parse_duration_from_str))]
    days: Duration,

    /// Which days of the week should be considered as a start of a journey
    #[structopt(short, long, parse(try_from_str = date::parse_weekday_from_str))]
    weekday: Option<Weekday>,

    /// Only consider outbound trains departing after this time
    #[structopt(long, value_name = "HH:MM", parse(try_from_str = date::parse_hour_from_str))]
    out_departure_after: Option<NaiveTime>,

    /// Only consider outbound trains departing before this time
    #[structopt(long, value_name = "HH:MM", parse(try_from_str = date::parse_hour_from_str))]
    out_departure_before: Option<NaiveTime>,

    /// Only consider inbound trains departing after this time
    #[structopt(long, value_name = "HH:MM", parse(try_from_str = date::parse_hour_from_str))]
    in_departure_after: Option<NaiveTime>,

    /// Only consider inbound trains departing before this time
    #[structopt(long, value_name = "HH:MM", parse(try_from_str = date::parse_hour_from_str))]
    in_departure_before: Option<NaiveTime>,

    /// Max price per journey
    #[structopt(short, long)]
    max_price: Option<f32>,

    /// Eurostar API key
    #[structopt(short, long)]
    api_key: String,

    /// How results should be sorted
    #[structopt(long, possible_values = &SortBy::variants(), case_insensitive = true, default_value = "price")]
    sort_by: SortBy,

    /// How many adults
    #[structopt(long, default_value = "1")]
    adults: i16,

    /// Start station
    #[structopt(default_value = "London")]
    from: String,

    /// Finish station
    #[structopt(default_value = "Paris")]
    to: String,
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();
    setup_logging(opt.verbose);

    debug!("Parsed opts: {:#?}", opt);

    if opt.from == opt.to {
        clap::Error::value_validation_auto(
            "Start and finish stations need to be different!".to_string(),
        )
        .exit();
    }

    let stations_map = get_stations_map(&opt.api_key).await.unwrap_or_else(|err| {
        error!("{:?}", err);
        std::process::exit(1);
    });

    let mut stations: [i32; 2] = [0, 0];
    for (i, station) in [opt.from, opt.to].iter().enumerate() {
        stations[i] = parse_station(&station, &stations_map)
            .unwrap_or_else(|err| clap::Error::value_validation_auto(err).exit());
    }

    let travels = date::get_possible_travel_dates(opt.since, opt.until, opt.days, opt.weekday)
        .unwrap_or_else(|err| clap::Error::value_validation_auto(err.to_string()).exit());

    if travels.is_empty() {
        clap::Error::value_validation_auto(
            "There are not dates pair matching your criteria!".to_string(),
        )
        .exit();
    } else {
        debug!("Possible travel dates: {:#?}", travels);
    }

    let filter = Filter {
        max_price: opt.max_price,
        out_departure_before: opt.out_departure_before,
        out_departure_after: opt.out_departure_after,
        in_departure_before: opt.in_departure_before,
        in_departure_after: opt.in_departure_after,
    };

    let journeys = match get_journeys(
        &travels,
        &opt.api_key,
        stations[0],
        stations[1],
        opt.adults,
        &filter,
    )
    .await
    {
        Ok(res) => res,
        Err(err) => {
            error!("{:?}", err);
            std::process::exit(1);
        }
    };

    if journeys.is_empty() {
        println!("There was no journey matching supplied criteria :(")
    } else {
        info!("Found {} journeys matching criteria.", journeys.len());
        format_results(journeys, opt.sort_by).printstd();
    }
}

fn format_results(mut journeys: Vec<TrainJourney>, sort_by: SortBy) -> Table {
    // Always pre-sort on price
    // journeys.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());

    match sort_by {
        SortBy::Price => journeys.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap()),
        SortBy::Date => journeys.sort_by(|a, b| a.outbound.cmp(&b.outbound)),
    }

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row!["Outbound (duration)", "Inbound (duration)", "Price"]);

    for journey in journeys.iter() {
        table.add_row(row![
            format!(
                "{} ({}h{}m)",
                journey.outbound.format(RESULT_DATETIME_FORMAT),
                journey.out_duration.num_hours(),
                journey.out_duration.num_minutes() % 60
            ),
            format!(
                "{} ({}h{}m)",
                journey.inbound.format(RESULT_DATETIME_FORMAT),
                journey.in_duration.num_hours(),
                journey.out_duration.num_minutes() % 60
            ),
            journey.price
        ]);
    }
    table
}

fn parse_station(name: &str, station_map: &HashMap<String, i32>) -> Result<i32, String> {
    match station_map.get(name) {
        Some(res) => Ok(*res),
        None => {
            let mut keys: Vec<&str> = station_map.keys().map(|val| val.as_str()).collect();
            keys.sort();
            Err(format!(
                "'{}' is invalid city name, choose from: {}.",
                name,
                keys.join(", ")
            ))
        }
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
