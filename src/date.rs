use chrono::{Duration, NaiveDate, NaiveTime, Utc};
use serde::{self, Deserialize, Deserializer};

static USER_FORMAT: &'static str = "%Y-%m-%d";
static TIME_FORMAT: &'static str = "%H:%M";
pub static NOW: &str = "now";
pub static PLUS_TWO_WEEKS: &str = "+2 weeks";

pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    NaiveTime::parse_from_str(&s, TIME_FORMAT).map_err(serde::de::Error::custom)
}

pub fn parse_date_from_str(date: &str) -> Result<NaiveDate, chrono::format::ParseError> {
    if date == NOW {
        return Ok(Utc::today().naive_local());
    } else if date == PLUS_TWO_WEEKS {
        return Ok(Utc::today().naive_local() + Duration::weeks(2));
    }
    NaiveDate::parse_from_str(date, USER_FORMAT)
}
