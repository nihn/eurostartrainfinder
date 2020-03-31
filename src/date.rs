use chrono::format::Numeric::WeekdayFromMon;
use chrono::{Duration, NaiveDate, NaiveTime, Utc, Weekday};
use serde::{self, Deserialize, Deserializer};
use std::fmt;

static USER_FORMAT: &'static str = "%Y-%m-%d";
static TIME_FORMAT: &'static str = "%H:%M";
pub static NOW: &str = "now";
pub static PLUS_TWO_WEEKS: &str = "+2 weeks";

#[derive(Debug)]
pub enum ParseError {
    ChronoError(chrono::format::ParseError),
    DateInThePastError(String),
    InvalidWeekday(String),
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

#[cfg(test)]
mod tests {
    use crate::date::parse_weekday_from_str;
    use chrono::Weekday;

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
}
