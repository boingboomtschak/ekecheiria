# ekecheiria

Proof-of-concept distributed image processing pipeline using GPU compute shaders. Written in Rust, uses [rumqttc](https://github.com/bytebeamio/rumqtt/tree/main/rumqttc) for MQTT client, and [wgpu](https://github.com/gfx-rs/wgpu) for GPU compute access.

## Usage

Run an MQTT broker (for local development, [rumqttd](https://github.com/bytebeamio/rumqtt/tree/main/rumqttd) is suggested) and configure it in the code as needed, otherwise it will connect to `localhost:1883`.

Run the below command on any number of consumers, which should subscribe them to the appropriate topics and initialize the wgpu instance.

```
cargo run --release --bin consumer
```

After the consumers are set up, run the below command to start the pipeline. The specified shader will be loaded, distributed to each consumer, and after a configurable number of seconds images will start to be sent.

```
cargo run --release --bin producer -- shaders/boxblur.wgsl
```

