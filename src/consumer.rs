#![allow(unused_imports)]
use std::{process, fs};
use log::{LevelFilter, debug, error, info, trace, warn};
use uuid::{Uuid};
use rumqttc::{Client, LastWill, MqttOptions, QoS, Event, Incoming, Outgoing};
use clap::Parser;
use pollster::FutureExt;
use image::{ImageBuffer, Rgba};

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    debug : bool
}

#[derive(Debug)]
struct GPU {
    device : wgpu::Device,
    queue : wgpu::Queue,
    pipeline : wgpu::ComputePipeline
}

impl GPU {
    fn init_pipeline(&self, shader : String) {
        debug!("Shader: {shader}");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mqtt-shader"),
            source: wgpu::ShaderSource::Wgsl(shader)
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("mqtt-pipeline"),
            layout: None
            module: &shader,
            entry_point: "main"
        });

    }

    fn process_image(&self, input_image : ImageBuffer<Rgba<u8>, Vec<u8>>) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let (width, height) = input_image.dimensions();
        let texture_size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let input_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("input-texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count : 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm]
        });
        self.queue.write_texture(
            input_texture.as_image_copy(),
            &input_image.as_raw(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: None
            },
            texture_size
        );
        let output_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output-texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm]
        });

        // create bind group layout

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("mqtt-pass")
        });
        compute_pass.set_pipeline(&self.pipeline);

        return input_image; // todo
    }
}

fn main() {
    let args = Args::parse(); 
    env_logger::builder().filter_level(if args.debug { LevelFilter::Debug } else { LevelFilter::Info }).init();
    debug!("CLI Args : {args:?}");

    let id = &Uuid::new_v4().to_string();
    info!("Starting consumer with id '{}'", id);
    let send_topic = "ekc-send-".to_owned() + id;

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
    let gpu = GPU { device : device, queue : queue };

    client.subscribe("ekc-init", QoS::AtLeastOnce).unwrap();
    client.subscribe(send_topic, QoS::AtLeastOnce).unwrap();

    for notification in connection.iter() {
        match notification {
            Ok(evt_dir) => match evt_dir {
                Event::Incoming(evt) => {
                    debug!("MQTT< {evt:?}");
                    if let Incoming::Publish(packet) = evt {
                        match packet.topic.as_str() {
                            "ekc-init" => {
                                gpu.init_pipeline(String::from_utf8(packet.payload.to_vec()).expect("Error reading shader from init event!"));
                                client.publish("ekc-reg", QoS::AtLeastOnce, false, id.as_bytes()).unwrap();
                            },
                            send_topic => {
                                let input_image = image::load_from_memory(&packet.payload.to_vec()).unwrap().to_rgba8();
                                let processed_image = gpu.process_image(input_image);
                                client.publish("ekc-recv", QoS::AtLeastOnce, false, vec![]).unwrap();
                            }
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