use chrono::NaiveDateTime;

use crate::query::Train;

#[derive(Debug)]
pub struct TrainJourney {
    pub outbound: NaiveDateTime,
    pub inbound: NaiveDateTime,
    pub price: f32,
}

pub fn filter_journeys(
    trains: (Vec<Train>, Vec<Train>),
    max_price: Option<f32>,
) -> Vec<TrainJourney> {
    let mut res = Vec::new();

    for out_t in trains.0.iter() {
        for in_t in trains.1.iter() {
            let total_price = out_t.price + in_t.price;
            if max_price.is_some() && total_price >= max_price.unwrap() {
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
