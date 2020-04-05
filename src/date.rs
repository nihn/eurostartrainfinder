use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Utc, Weekday};
use serde::{self, Deserialize, Deserializer};
use std::fmt;
use std::num::ParseIntError;

static USER_FORMAT: &'static str = "%Y-%m-%d";
static TIME_FORMAT: &'static str = "%H:%M";
pub static NOW: &str = "now";
pub static PLUS_TWO_WEEKS: &str = "+2 weeks";

#[derive(Debug, PartialEq)]
pub enum ParseError {
    ChronoError(chrono::format::ParseError),
    DateInThePastError(String),
    InvalidWeekday(String),
    ParseIntError(ParseIntError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::ChronoError(err) => write!(f, "{}", err),
            msg => write!(f, "{}", msg),
        }
    }
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    NaiveTime::parse_from_str(&s, TIME_FORMAT).map_err(serde::de::Error::custom)
}

pub fn parse_duration_from_str(days: &str) -> Result<Duration, ParseError> {
    match days.parse::<i64>() {
        Ok(res) => {
            if res >= 1 {
                Ok(Duration::days(res - 1))
            } else {
                Err(ParseError::DateInThePastError(
                    "Number of days must be greater than 0!".to_string(),
                ))
            }
        }
        Err(err) => Err(ParseError::ParseIntError(err)),
    }
}

pub fn parse_date_from_str(date: &str) -> Result<NaiveDate, ParseError> {
    if date == NOW {
        return Ok(Utc::today().naive_local());
    } else if date == PLUS_TWO_WEEKS {
        return Ok(Utc::today().naive_local() + Duration::weeks(2));
    }
    let parsed = NaiveDate::parse_from_str(date, USER_FORMAT).map_err(ParseError::ChronoError)?;
    if parsed < Utc::today().naive_local() {
        return Err(ParseError::DateInThePastError(format!(
            "{:?} is in the past!",
            parsed
        )));
    }
    Ok(parsed)
}

pub fn parse_weekday_from_str(weekday: &str) -> Result<Weekday, ParseError> {
    match weekday.to_lowercase().as_str() {
        "monday" => Ok(Weekday::Mon),
        "tuesday" => Ok(Weekday::Tue),
        "wednesday" => Ok(Weekday::Wed),
        "thursday" => Ok(Weekday::Thu),
        "friday" => Ok(Weekday::Fri),
        "saturday" => Ok(Weekday::Sat),
        "sunday" => Ok(Weekday::Sun),
        day => Err(ParseError::InvalidWeekday(format!(
            "{} is an invalid weekday name!",
            day
        ))),
    }
}

pub fn get_possible_travel_dates(
    from: NaiveDate,
    until: NaiveDate,
    days: Duration,
    weekday: Option<Weekday>,
) -> Result<Vec<(NaiveDate, NaiveDate)>, &'static str> {
    let mut results = Vec::new();

    let mut outbound = match weekday {
        Some(weekday) => {
            let mut day = from;
            while day.weekday() != weekday {
                day = day.succ();
            }
            day
        }
        None => from,
    };
    let mut inbound = outbound + days;

    if inbound > until {
        return Err("There is no possible out-inbound dates which could satisfy your query");
    };

    while inbound <= until {
        results.push((outbound, inbound));
        outbound = match weekday {
            Some(_) => outbound + Duration::days(7),
            None => outbound.succ(),
        };
        inbound = outbound + days;
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use crate::date::{
        get_possible_travel_dates, parse_date_from_str, parse_duration_from_str,
        parse_weekday_from_str, ParseError, NOW, PLUS_TWO_WEEKS,
    };
    use chrono::{Duration, NaiveDate, Utc, Weekday};

    #[test]
    fn test_duration_from_str() {
        assert_eq!(parse_duration_from_str("3").unwrap(), Duration::days(2),)
    }

    #[test]
    fn test_duration_from_str_below_zero() {
        assert_eq!(
            parse_duration_from_str("-1").unwrap_err(),
            ParseError::DateInThePastError("Number of days must be greater than 0!".to_string()),
        )
    }

    #[test]
    fn test_duration_from_str_invalid_number() -> Result<(), String> {
        match parse_duration_from_str("foo") {
            Err(ParseError::ParseIntError(_)) => Ok(()),
            _ => Err("Should raise ParseError".to_string()),
        }
    }

    #[test]
    fn test_parse_date_from_str_now() {
        assert_eq!(
            parse_date_from_str(NOW).unwrap(),
            Utc::today().naive_local()
        );
    }

    #[test]
    fn test_parse_date_from_str_two_weeks_in_the_future() {
        assert_eq!(
            parse_date_from_str(PLUS_TWO_WEEKS).unwrap(),
            Utc::today().naive_local() + Duration::weeks(2)
        );
    }

    #[test]
    fn test_parse_date_from_str() {
        let today = Utc::now().naive_local().date();
        assert_eq!(
            parse_date_from_str(&today.format("%Y-%m-%d").to_string()).unwrap(),
            today,
        );
    }

    #[test]
    fn test_parse_date_from_str_in_the_past() {
        assert_eq!(
            parse_date_from_str("2020-03-30").unwrap_err(),
            ParseError::DateInThePastError("2020-03-30 is in the past!".to_string()),
        )
    }

    #[test]
    fn test_parse_date_from_str_invalid() -> Result<(), String> {
        match parse_date_from_str("foo") {
            Err(ParseError::ChronoError(_)) => Ok(()),
            _ => Err("Should fail with ParseError::ChronoError".to_string()),
        }
    }

    #[test]
    fn test_parse_weekday_from_str() {
        let cases = vec![
            ("monday", Weekday::Mon),
            ("Tuesday", Weekday::Tue),
            ("WEDNESDAY", Weekday::Wed),
            ("thursDay", Weekday::Thu),
            ("friday", Weekday::Fri),
            ("Saturday", Weekday::Sat),
            ("SUNDAY", Weekday::Sun),
        ];

        for (string, weekday) in cases.iter() {
            assert_eq!(&parse_weekday_from_str(string).unwrap(), weekday)
        }
    }

    #[test]
    fn test_parse_weekday_from_str_invalid() {
        assert!(parse_weekday_from_str("invalid").is_err())
    }

    #[test]
    fn test_get_possible_travel_dates_impossible_constraints() {
        let msg = "There is no possible out-inbound dates which could satisfy your query";
        match get_possible_travel_dates(
            NaiveDate::from_ymd(2020, 1, 1),
            NaiveDate::from_ymd(2020, 1, 3),
            Duration::days(3),
            None,
        ) {
            Err(err) => assert_eq!(err, msg),
            _ => panic!("get_possible_travel_dates should have return error!"),
        };

        match get_possible_travel_dates(
            NaiveDate::from_ymd(2020, 4, 1),
            NaiveDate::from_ymd(2020, 4, 10),
            Duration::days(6),
            Some(Weekday::Tue),
        ) {
            Err(err) => assert_eq!(err, msg),
            _ => panic!("get_possible_travel_dates should have return error!"),
        };
    }

    #[test]
    fn test_get_possible_travel_dates_ok() {
        assert_eq!(
            get_possible_travel_dates(
                NaiveDate::from_ymd(2020, 1, 1),
                NaiveDate::from_ymd(2020, 1, 6),
                Duration::days(3),
                None,
            )
            .unwrap(),
            vec![
                (
                    NaiveDate::from_ymd(2020, 1, 1),
                    NaiveDate::from_ymd(2020, 1, 4)
                ),
                (
                    NaiveDate::from_ymd(2020, 1, 2),
                    NaiveDate::from_ymd(2020, 1, 5)
                ),
                (
                    NaiveDate::from_ymd(2020, 1, 3),
                    NaiveDate::from_ymd(2020, 1, 6)
                ),
            ]
        );

        assert_eq!(
            get_possible_travel_dates(
                NaiveDate::from_ymd(2020, 4, 1),
                NaiveDate::from_ymd(2020, 4, 6),
                Duration::days(3),
                Some(Weekday::Wed),
            )
            .unwrap(),
            vec![(
                NaiveDate::from_ymd(2020, 4, 1),
                NaiveDate::from_ymd(2020, 4, 4)
            ),]
        )
    }
}
