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
use crate::shared::EkcImage;

mod shared;

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

    // Load images from image path, create processed directory if needed
    let image_paths = fs::read_dir(IMAGES_PATH)
        .expect("Failed to load images from path")
        .map(|entry| { entry.unwrap().path() })
        .filter(|path| !path.ends_with(".DS_Store"))
        .collect::<Vec<PathBuf>>();
    let num_images = image_paths.len();
    info!("Found {num_images} images in '{IMAGES_PATH}'");
    let mut images_processed = 0;
    std::fs::create_dir_all(PROCESSED_PATH).expect("Failed to create processed image directory");
    
    let id = &Uuid::new_v4().to_string();
    info!("Starting producer with id '{}'", id);

    // Configure and initialize MQTT client with connection
    let mut mqttoptions = MqttOptions::new(id, "localhost", 1883);
    mqttoptions.set_max_packet_size(128000000, 128000000);
    let (mqtt_client, mut connection) = Client::new(mqttoptions, 10);
    let client = Arc::new(mqtt_client);
    let t_client = client.clone();

    // Subscribe to registration topic, then send shader to any listening consumers
    client.subscribe("ekc-reg", QoS::AtLeastOnce).unwrap();
    client.publish("ekc-init", QoS::AtLeastOnce, false, shader.as_bytes()).unwrap();

    // Create consumers hashmap, with multiple references for main / image sending thread
    let consumers = Arc::new(Mutex::new(HashMap::<String, ConsumerStatus>::new()));
    let t_consumers = consumers.clone();

    // Spawn new task to asynchronously send images after number of seconds
    task::spawn(async move {
        let secs = 3;
        info!("Starting to send images in {secs} seconds...");
        task::sleep(Duration::from_secs(secs)).await;

        info!("Beginning to send images...");
        for image_path in image_paths.iter() {
            info!("Sending '{image_path:?}'");
            
            // Find next available consumer, spin until one is ready
            let mut consumer : Option<String> = None;
            while consumer.is_none() {
                for (key, val) in t_consumers.lock().expect("Couldn't lock consumers").iter() {
                    if val == &ConsumerStatus::Ready {
                        consumer = Some(key.to_string());
                        break;
                    }
                }
            }

            // Load the image file, serialize it, send to consumer
            let send_topic = "ekc-send-".to_owned() + consumer.as_ref().unwrap();
            let image = image::io::Reader::open(image_path).expect("Failed to read image").decode().expect("Failed to decode image").into_rgba8();
            let image_payload = EkcImage { width: image.width(), height: image.height(), image_data: image.into_raw() };
            t_client.publish(send_topic, QoS::AtLeastOnce, false, image_payload).expect("Failed to send image");

            // Mark consumer as processing in hashmap
            t_consumers.lock().expect("Couldn't lock consumers").insert(consumer.unwrap(), ConsumerStatus::Processing);
        }
    });

    // Process incoming events from MQTT connection
    for notification in connection.iter() {
        match notification {
            Ok(evt_dir) => match evt_dir {
                Event::Incoming(evt) => {
                    debug!("MQTT< {evt:?}");
                    // Handle incoming publish packets
                    if let Incoming::Publish(packet) = evt {
                        if packet.topic == "ekc-reg" {
                            // Handle new consumer being registered, enter to hashmap and subscribe to recv topic
                            let id = String::from_utf8(packet.payload.to_vec()).expect("Error parsing consumer id from reg");
                            consumers.lock().expect("Couldn't lock consumers").insert(id.clone(), ConsumerStatus::Ready);
                            client.subscribe("ekc-recv-".to_owned() + &id, QoS::AtLeastOnce).unwrap();
                            info!("Registered consumer '{id}'")

                        } else if packet.topic.starts_with("ekc-recv-") {
                            // Handle image being received from consumer

                            // Update consumer in hashmap back to ready
                            let consumer_id = packet.topic.strip_prefix("ekc-recv-").expect("Failed to parse consumer id");
                            consumers.lock().expect("Couldn't lock consumers").insert(consumer_id.to_string(), ConsumerStatus::Ready); 

                            // Parse packet payload, deserialize data, load as image, and save
                            let processed_payload = EkcImage::try_from(packet.payload.as_ref()).expect("Error deserializing image");
                            let processed_image = image::RgbaImage::from_raw(processed_payload.width, processed_payload.height, processed_payload.image_data).expect("Error loading image from raw");
                            images_processed += 1;
                            processed_image.save(format!("{PROCESSED_PATH}/image{images_processed}.png")).expect("Failed to save processed image");
                            info!("Received and saved 'image{images_processed}.png'");

                            // Disconnect client and end event loop if all images processed
                            if images_processed == num_images {
                                client.disconnect().expect("Failed to disconnect MQTT client");
                                break;
                            }
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
