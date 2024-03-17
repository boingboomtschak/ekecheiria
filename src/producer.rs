use std::fs;
use async_std::task;
use std::time::Duration;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use log::{LevelFilter, debug, error, info};
use uuid::{Uuid};
use rumqttc::{Client, MqttOptions, QoS, Event, Incoming};
use clap::Parser;
use image::ImageBuffer;

const IMAGES_PATH : &str = "images";
const PROCESSED_PATH : &str = "processed_images";

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg()]
    shader : String,

    #[arg(short, long)]
    debug : bool
}

#[derive(Debug, PartialEq)]
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

    let image_paths = fs::read_dir(IMAGES_PATH)
        .expect("Failed to load images from path")
        .map(|entry| { entry.unwrap().path() })
        .collect::<Vec<PathBuf>>();
    info!("Found {0} images in '{IMAGES_PATH}'", image_paths.len());
    let mut images_processed = 0;
    
    let id = &Uuid::new_v4().to_string();
    info!("Starting producer with id '{}'", id);

    let mut mqttoptions = MqttOptions::new(id, "localhost", 1883);
    mqttoptions.set_max_packet_size(32000000, 32000000);
    let (mqtt_client, mut connection) = Client::new(mqttoptions, 10);
    let client = Arc::new(mqtt_client);
    let t_client = client.clone();

    client.subscribe("ekc-reg", QoS::AtLeastOnce).unwrap();
    client.publish("ekc-init", QoS::AtLeastOnce, false, shader.as_bytes()).unwrap();

    let consumers = Arc::new(Mutex::new(HashMap::<String, ConsumerStatus>::new()));
    let t_consumers = consumers.clone();

    task::spawn(async move {
        info!("Starting to send images in 15 seconds...");
        task::sleep(Duration::from_secs(15)).await;
        info!("Beginning to send images...");
        for image_path in image_paths.iter() {
            info!("Sending '{image_path:?}'");
            let mut consumer : Option<String> = None;
            while consumer.is_none() {
                for (key, val) in t_consumers.lock().expect("Couldn't lock consumers").iter() {
                    if val == &ConsumerStatus::Ready {
                        consumer = Some(key.to_string());
                        break;
                    }
                }
            }
            let send_topic = "ekc-send-".to_owned() + consumer.as_ref().unwrap();
            let image = image::io::Reader::open(image_path).expect("Failed to read image").decode().expect("Failed to decode image").into_rgba8();
            t_client.publish(send_topic, QoS::AtLeastOnce, false, image.into_raw());
        }
    });

    for notification in connection.iter() {
        match notification {
            Ok(evt_dir) => match evt_dir {
                Event::Incoming(evt) => {
                    debug!("MQTT< {evt:?}");
                    if let Incoming::Publish(packet) = evt {
                        if packet.topic == "ekc-reg" {
                            let id = String::from_utf8(packet.payload.to_vec()).expect("Error parsing consumer id from reg");
                            consumers.lock().expect("Couldn't lock consumers").insert(id.clone(), ConsumerStatus::Ready);
                            client.subscribe("ekc-recv-".to_owned() + &id, QoS::AtLeastOnce).unwrap();
                            info!("Registered consumer '{id}'")
                        } else if packet.topic.starts_with("ekc-recv") {
                            let processed_image = image::load_from_memory(&packet.payload.to_vec()).unwrap().to_rgba8();
                            images_processed += 1;
                            processed_image.save(format!("{PROCESSED_PATH}/image{images_processed}.png"));
                            info!("Processed and saved 'image{images_processed}.png'");
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
