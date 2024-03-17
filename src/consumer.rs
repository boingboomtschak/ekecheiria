use log::{LevelFilter, debug, error, info};
use uuid::{Uuid};
use rumqttc::{Client, MqttOptions, QoS, Event, Incoming};
use clap::Parser;
use pollster::FutureExt;
use image::{ImageBuffer, Rgba};
use crate::shared::EkcImage;

mod shared;

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
    pipeline : Option<wgpu::ComputePipeline>
}

const WORKGROUP_SIZE_X : u32 = 16;
const WORKGROUP_SIZE_Y : u32 = 16;


impl GPU {
    fn init_pipeline(&mut self, shader : String) {
        debug!("Shader: {shader}");
        let shader_module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mqtt-shader"),
            source: wgpu::ShaderSource::Wgsl(shader.into())
        });

        self.pipeline = Some(self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("mqtt-pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: "main"
        }));
    }

    fn padded_bytes_per_row(width: u32) -> u32 {
        let bytes_per_row = width * 4;
        let padding = (256 - bytes_per_row % 256) % 256;
        return bytes_per_row + padding;
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

        let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mqtt-bind-group"),
            layout: &self.pipeline.as_ref().expect("Creating bind group from uninitialized pipeline").get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { 
                    binding: 0, 
                    resource: wgpu::BindingResource::TextureView(&input_texture.create_view(&wgpu::TextureViewDescriptor::default()))
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&output_texture.create_view(&wgpu::TextureViewDescriptor::default()))
                }
            ]
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mqtt-pass"),
                timestamp_writes: None
            });
            compute_pass.set_pipeline(&self.pipeline.as_ref().expect("Compute pass setting uninitialized pipeline"));
            compute_pass.set_bind_group(0, &texture_bind_group, &[]);
            compute_pass.dispatch_workgroups((width + WORKGROUP_SIZE_X - 1) / WORKGROUP_SIZE_X, (height + WORKGROUP_SIZE_Y - 1) / WORKGROUP_SIZE_Y, 1);
        }

        let padded_bytes_per_row = GPU::padded_bytes_per_row(width);
        let unpadded_bytes_per_row = width * 4;
        let output_buffer_size = (padded_bytes_per_row * height) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height)
                }
            },
            texture_size
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = output_buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

        self.device.poll(wgpu::Maintain::Wait);

        let padded_data = buffer_slice.get_mapped_range();
        let mut pixels: Vec<u8> = vec![0; (unpadded_bytes_per_row * height) as usize];
        for (padded, pixels) in padded_data
            .chunks_exact(padded_bytes_per_row as usize)
            .zip(pixels.chunks_exact_mut(unpadded_bytes_per_row as usize)) {
            pixels.copy_from_slice(&padded[..(unpadded_bytes_per_row as usize)])
        }

        let output_image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, pixels).expect("Failed to create output image from pixels");
        return output_image; 
    }
}

fn main() {
    let args = Args::parse(); 
    env_logger::builder().filter_level(if args.debug { LevelFilter::Debug } else { LevelFilter::Info }).init();
    debug!("CLI Args : {args:?}");

    let id = &Uuid::new_v4().to_string();
    info!("Starting consumer with id '{}'", id);
    let send_topic = "ekc-send-".to_owned() + id;
    let recv_topic = "ekc-recv-".to_owned() + id;

    let mut mqttoptions = MqttOptions::new(id, "localhost", 1883);
    mqttoptions.set_max_packet_size(32000000, 32000000);
    let (client, mut connection) = Client::new(mqttoptions, 10);

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
    let mut gpu = GPU { device : device, queue : queue, pipeline : None };

    client.subscribe("ekc-init", QoS::AtLeastOnce).unwrap();
    client.subscribe(send_topic.clone(), QoS::AtLeastOnce).unwrap();

    for notification in connection.iter() {
        match notification {
            Ok(evt_dir) => match evt_dir {
                Event::Incoming(evt) => {
                    debug!("MQTT< {evt:?}");
                    if let Incoming::Publish(packet) = evt {
                        if packet.topic == "ekc-init" {
                            gpu.init_pipeline(String::from_utf8(packet.payload.to_vec()).expect("Error reading shader from init event!"));
                            client.publish("ekc-reg", QoS::AtLeastOnce, false, id.as_bytes()).unwrap();
                            info!("Registered with producer");
                        } else if packet.topic == send_topic {
                            let image_payload = EkcImage::try_from(packet.payload.as_ref()).expect("Error deserializing image");
                            let input_image = image::RgbaImage::from_raw(image_payload.width, image_payload.height, image_payload.image_data).expect("Error loading image from raw");
                            let processed_image = gpu.process_image(input_image);
                            let processed_payload = EkcImage { width: processed_image.width(), height: processed_image.height(), image_data: processed_image.into_raw() };
                            client.publish(recv_topic.clone(), QoS::AtLeastOnce, false, processed_payload).unwrap();
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