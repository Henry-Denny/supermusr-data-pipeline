use chrono::Utc;
use clap::Parser;
use common::Intensity;
use rdkafka::{
    config::ClientConfig,
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
};
use std::time::{Duration, SystemTime};
use streaming_types::{
    dat1_digitizer_analog_trace_v1_generated::{
        finish_digitizer_analog_trace_message_buffer, ChannelTrace, ChannelTraceArgs,
        DigitizerAnalogTraceMessage, DigitizerAnalogTraceMessageArgs,
    },
    flatbuffers::{FlatBufferBuilder, Vector, WIPOffset},
    frame_metadata_v1_generated::{FrameMetadataV1, FrameMetadataV1Args, GpsTime},
};
use tokio::time;

#[derive(Clone, Parser)]
#[clap(author, version, about)]
struct Cli {
    /// Kafka broker address
    #[clap(long = "broker")]
    broker_address: String,

    /// Kafka username
    #[clap(long)]
    username: String,

    /// Kafka password
    #[clap(long)]
    password: String,

    /// Topic to publish analog trace packets to
    #[clap(long)]
    trace_topic: String,

    /// Digitizer identifier to use
    #[clap(long = "did", default_value = "0")]
    digitizer_id: u8,

    /// Number of measurements to include in each frame
    #[clap(long = "time-bins", default_value = "20000")]
    measurements_per_frame: usize,

    /// Number of first frame to be sent
    #[clap(long = "start-frame", default_value = "0")]
    start_frame_number: u32,

    /// Time in milliseconds between each frame
    #[clap(long, default_value = "20")]
    frame_time: u64,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let cli = Cli::parse();

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cli.broker_address)
        .set("security.protocol", "sasl_plaintext")
        .set("sasl.mechanisms", "SCRAM-SHA-256")
        .set("sasl.username", &cli.username)
        .set("sasl.password", &cli.password)
        .create()
        .unwrap();

    let mut fbb = FlatBufferBuilder::new();

    let mut frame = time::interval(Duration::from_millis(cli.frame_time));

    let mut frame_number = cli.start_frame_number;

    let mut channel_voltage_data: Vec<Vec<Intensity>> = Vec::default();
    for _ in 0..8 {
        let mut data = Vec::<Intensity>::new();
        data.resize(cli.measurements_per_frame, 404);
        data[1] = cli.digitizer_id as Intensity;
        channel_voltage_data.push(data);
    }

    loop {
        fbb.reset();

        let time: GpsTime = Utc::now().into();

        let metadata = FrameMetadataV1Args {
            frame_number,
            period_number: 0,
            protons_per_pulse: 0,
            running: true,
            timestamp: Some(&time),
            veto_flags: 0,
        };
        let metadata = FrameMetadataV1::create(&mut fbb, &metadata);

        let mut channel_voltage_vectors: Vec<Option<WIPOffset<Vector<Intensity>>>> = Vec::default();
        let mut channels: Vec<WIPOffset<ChannelTrace>> = Vec::default();
        for i in 0..8 {
            channel_voltage_data[i][0] = frame_number as Intensity;

            channel_voltage_vectors.push(Some(
                fbb.create_vector::<Intensity>(&channel_voltage_data[i]),
            ));

            channels.push(ChannelTrace::create(
                &mut fbb,
                &ChannelTraceArgs {
                    channel: i as u32,
                    voltage: channel_voltage_vectors[i],
                },
            ));
        }

        let message = DigitizerAnalogTraceMessageArgs {
            digitizer_id: cli.digitizer_id,
            metadata: Some(metadata),
            sample_rate: 1_000_000_000,
            channels: Some(fbb.create_vector(&channels)),
        };
        let message = DigitizerAnalogTraceMessage::create(&mut fbb, &message);
        finish_digitizer_analog_trace_message_buffer(&mut fbb, message);

        let start_time = SystemTime::now();

        match producer
            .send(
                FutureRecord::to(&cli.trace_topic)
                    .payload(fbb.finished_data())
                    .key(&"todo".to_string()),
                Timeout::After(Duration::from_millis(100)),
            )
            .await
        {
            Ok(r) => log::debug!("Delivery: {:?}", r),
            Err(e) => log::error!("Delivery failed: {:?}", e),
        };

        log::info!(
            "Trace send took: {:?}",
            SystemTime::now().duration_since(start_time).unwrap()
        );

        frame_number += 1;
        frame.tick().await;
    }
}
