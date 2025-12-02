#![forbid(unsafe_code)]

pub mod futures {
    pub use futures::*;
}

pub mod futures_timer {
    pub use futures_timer::*;
}

pub mod core;
pub mod utils;
pub use mycelium_computing_macros::*;

extern crate self as mycelium_computing;
