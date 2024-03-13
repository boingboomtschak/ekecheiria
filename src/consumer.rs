#![allow(unused_imports)]
use std::process;
use log::{LevelFilter, debug, error, info, trace, warn};
use uuid::{Uuid};
use rumqttc::{Client, LastWill, MqttOptions, QoS};

fn main() {
    env_logger::builder().filter_level(LevelFilter::max()).init();
    let id = "consumer:".to_owned() + &Uuid::new_v4().to_string();
    info!("Starting consumer with id '{}'", id);

    let mqttoptions = MqttOptions::new(id, "localhost", 1883);
    let (mut client, mut connection) = Client::new(mqttoptions, 10);

    client.subscribe("ekc-send", QoS::AtLeastOnce);

    for (i, notification) in connection.iter().enumerate() {
        match notification {
            Ok(notif) => {
                info!("{i}. Notification = {notif:?}");
            }
            Err(error) => {
                error!("{i}. Notification = {error:?}");
                return;
            }
        }
    }
    info!("Ending consumer...");
}
