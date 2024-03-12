#![allow(unused_imports)]
use std::process;
use log::{LevelFilter, debug, error, info, trace, warn};

fn main() {
    env_logger::builder().filter_level(LevelFilter::max()).init();

    info!("starting producer!");
}
