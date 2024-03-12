#![allow(unused_imports)]
use std::process;
use log::{LevelFilter, debug, error, info, trace, warn};
use paho_mqtt as mqtt;

fn main() {
    env_logger::builder().filter_level(LevelFilter::max()).init();

    info!("starting consumer!");
}