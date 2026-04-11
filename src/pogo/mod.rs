//! Pokémon GO event and promo code data from leekduck.com (via ScrapedDuck API).
//!
//! Data source: https://raw.githubusercontent.com/bigfoott/ScrapedDuck/data/events.min.json
//! This JSON powers leekduck.com and is updated frequently.

mod types;
mod fetch;

pub use types::{PogoEvent, PromoCode, PogoData};
