#![allow(unused_imports)]
use std::{process, fs};
use log::{LevelFilter, debug, error, info, trace, warn};
use uuid::{Uuid};
use rumqttc::{Client, LastWill, MqttOptions, QoS, Event, Incoming, Outgoing};
use clap::Parser;
use pollster::FutureExt;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    debug : bool
}


fn main() {
    let args = Args::parse(); 
    env_logger::builder().filter_level(if args.debug { LevelFilter::Debug } else { LevelFilter::Info }).init();
    debug!("CLI Args : {args:?}");

    let id = &Uuid::new_v4().to_string();
    info!("Starting consumer with id '{}'", id);

    let mqttoptions = MqttOptions::new(id, "localhost", 1883);
    let (mut client, mut connection) = Client::new(mqttoptions, 10);

    let instance = wgpu::Instance::default();
    let adapter = instance.request_adapter(
        &wgpu::RequestAdapterOptionsBase {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None
        })
        .block_on().expect("Failed to request adapter from instance");
    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .block_on().expect("Failed to receive device from adapter");

    client.subscribe("ekc-init", QoS::AtLeastOnce).unwrap();
    client.subscribe("ekc-send-".to_owned() + id, QoS::AtLeastOnce).unwrap();

    for notification in connection.iter() {
        match notification {
            Ok(evt_dir) => match evt_dir {
                Event::Incoming(evt) => {
                    debug!("MQTT< {evt:?}");
                    if let Incoming::Publish(packet) = evt {
                        match packet.topic.as_str() {
                            "ekc-init" => {
                                init_pipeline(String::from_utf8(packet.payload.to_vec()).expect("Error reading shader from init event!"));
                                client.publish("ekc-reg", QoS::AtLeastOnce, false, id.as_bytes());
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
    info!("Ending consumer...");
}

fn init_pipeline(shader : String) {
    debug!("Shader: {shader}");
}

fn process_image() {

}