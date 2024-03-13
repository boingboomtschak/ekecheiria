#![allow(unused_imports)]
use std::process;
use log::{LevelFilter, debug, error, info, trace, warn};
use uuid::{Uuid};
use rumqttc::{Client, LastWill, MqttOptions, QoS};
use clap::Parser;

const IMAGES_PATH : &str = "images/";
const PROCESSED_PATH : &str = "processed_images/";

#[derive(Parser, Debug)]
#[command(name = "ekecheiria-producer")]
struct Args {
    #[arg()]
    shader : String
}

/*
- producer loads job shader from file
- producer publishes (synchronously) to send topic with job shader as payload
- consumer(s) see publish, load job shader, set up pipeline, then publish to reg topic with their id and status
- producer asynchronously reads status msgs from reg topic, saves list of ready 
- producer reads list of images from folder, then for each:
    - loads image as bytes, converts to necessary format
    - finds next ready consumer in registered consumers
    - publishes image as payload to send topic for consumer (likely send topic with uuid appended)
    - marks them as processing
    - asynchronously waits for recv back from consumer, which includes processed image as payload
    - marks consumer as ready again
*/

fn main() {
    env_logger::builder().filter_level(LevelFilter::max()).init();
    let args = Args::parse(); 
    debug!("args : {args:?}");
    
    let id = "producer:".to_owned() + &Uuid::new_v4().to_string();
    info!("Starting producer with id '{}'", id);

    let mqttoptions = MqttOptions::new(id, "localhost", 1883);
    let (mut client, mut connection) = Client::new(mqttoptions, 10);

    client.publish("ekc-send", QoS::AtLeastOnce, false, vec![]).unwrap();
    client.subscribe("ekc-reg", QoS::AtLeastOnce).unwrap();
    client.subscribe("ekc-recv", QoS::AtLeastOnce).unwrap();


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
    info!("Ending producer...");
}
