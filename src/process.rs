use chrono::NaiveDateTime;

use crate::query::Train;

#[derive(Debug)]
pub struct TrainJourney {
    outbound: NaiveDateTime,
    inbound: NaiveDateTime,
    price: f32,
}

pub fn get_journeys(
    trains: Vec<(Vec<Train>, Vec<Train>)>,
    max_price: Option<f32>,
) -> Vec<TrainJourney> {
    let mut res = Vec::new();

    for pair in trains.iter() {
        for out_t in pair.0.iter() {
            for in_t in pair.1.iter() {
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
    }
    res
}
