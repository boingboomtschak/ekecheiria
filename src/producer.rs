#![allow(unused_imports)]
use std::{process, fs};
use std::collections::HashMap;
use log::{LevelFilter, debug, error, info, trace, warn};
use uuid::{Uuid};
use rumqttc::{Client, LastWill, MqttOptions, QoS, Event, Incoming, Outgoing};
use clap::Parser;

const IMAGES_PATH : &str = "images/";
const PROCESSED_PATH : &str = "processed_images/";

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg()]
    shader : String,

    #[arg(short, long)]
    debug : bool
}

#[derive(Debug)]
enum ConsumerStatus {
    Ready,
    Processing
}

/*
- producer loads job shader from file (*)
- producer publishes (synchronously) to init topic with job shader as payload
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
    let args = Args::parse(); 
    env_logger::builder().filter_level(if args.debug { LevelFilter::Debug } else { LevelFilter::Info }).init();
    debug!("CLI Args : {args:?}");

    info!("Loading shader '{0}'...", args.shader);
    let shader = fs::read_to_string(args.shader).expect("Bad path to shader file");
    
    let id = "producer:".to_owned() + &Uuid::new_v4().to_string();
    info!("Starting producer with id '{}'", id);

    let mqttoptions = MqttOptions::new(id, "localhost", 1883);
    let (mut client, mut connection) = Client::new(mqttoptions, 10);

    client.publish("ekc-init", QoS::AtLeastOnce, false, shader.as_bytes()).unwrap();
    client.subscribe("ekc-reg", QoS::AtLeastOnce).unwrap();
    client.subscribe("ekc-recv", QoS::AtLeastOnce).unwrap();

    let mut consumers : HashMap<String, ConsumerStatus> = HashMap::new();

    for notification in connection.iter() {
        match notification {
            Ok(evt_dir) => match evt_dir {
                Event::Incoming(evt) => {
                    debug!("MQTT< {evt:?}");
                    if let Incoming::Publish(packet) = evt {
                        match packet.topic.as_str() {
                            "ekc-reg" => {
                                consumers.insert(String::from_utf8(packet.payload.to_vec()).expect("Error parsing consumer id from reg"), ConsumerStatus::Ready);
                                debug!("Consumers: {consumers:?}");
                            },
                            _ => ()
                        }
                    }
                },
                Event::Outgoing(evt) => {
                    debug!("MQTT> {evt:?}");
                }
            },
            Err(evt) => {
                error!("MQTT! {evt:?}");
            }
        }
    }
    info!("Ending producer...");
}
