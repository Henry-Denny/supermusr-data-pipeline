//! This crate uses the benchmarking tool for testing the performance of implementated time-series databases.
//!

mod tdengine;

use clap::Parser;
use rdkafka::{
    consumer::{stream_consumer::StreamConsumer, CommitMode, Consumer},
    message::Message,
};
use supermusr_common::tracer::OtelTracer;
use supermusr_common::{conditional_init_tracer, tracer::OptionalHeaderTracerExt};
use supermusr_streaming_types::dat2_digitizer_analog_trace_v2_generated::{
    digitizer_analog_trace_message_buffer_has_identifier, root_as_digitizer_analog_trace_message,
};
use tdengine::{wrapper::TDEngine, TimeSeriesEngine};
use tracing::{debug, info, level_filters::LevelFilter, trace_span, warn};

#[derive(Parser)]
#[clap(author, version, about)]
pub(crate) struct Cli {
    /// The kafka broker to use e.g. --broker localhost:19092
    #[clap(long)]
    kafka_broker: String,

    /// Optional Kafka username
    #[clap(long)]
    kafka_username: Option<String>,

    /// Optional Kafka password
    #[clap(long)]
    kafka_password: Option<String>,

    /// Kafka consumer group e.g. --kafka_consumer_group trace-producer
    #[clap(long)]
    kafka_consumer_group: String,

    /// Kafka topic e.g. --kafka-topic Traces
    #[clap(long)]
    kafka_topic: String,

    /// TDengine dsn  e.g. --td_dsn localhost:6041
    #[clap(long)]
    td_dsn: String,

    /// Optional TDengine username
    #[clap(long)]
    td_username: Option<String>,

    /// Optional TDengine password
    #[clap(long)]
    td_password: Option<String>,

    /// TDengine database name e.g. --td_database tracelogs
    #[clap(long)]
    td_database: String,

    /// Number of expected channels in a message e.g. --num_channels 8
    #[clap(long)]
    num_channels: usize,

    /// If set, then open-telemetry data is sent to the URL specified, otherwise the standard tracing subscriber is used
    #[clap(long)]
    otel_endpoint: Option<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let tracer = conditional_init_tracer!(cli.otel_endpoint.as_deref(), LevelFilter::TRACE);

    debug!("Creating TDEngine instance");
    let mut tdengine: TDEngine = TDEngine::from_optional(
        cli.td_dsn,
        cli.td_username,
        cli.td_password,
        cli.td_database,
    )
    .await
    .expect("TDengine should be created");

    //  All other modes require the TDEngine to be initialised
    tdengine
        .create_database()
        .await
        .expect("TDengine database should be created");
    tdengine
        .init_with_channel_count(cli.num_channels)
        .await
        .expect("TDengine should initialise with given channel count");

    //  All other modes require a kafka builder, a topic, and redpanda consumer
    debug!("Creating Kafka instance");
    let mut client_config = supermusr_common::generate_kafka_client_config(
        &cli.kafka_broker,
        &cli.kafka_username,
        &cli.kafka_password,
    );

    let consumer: StreamConsumer = client_config
        .set("group.id", &cli.kafka_consumer_group)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "false")
        .create()
        .expect("Kafka Consumer should be created");
    consumer
        .subscribe(&[&cli.kafka_topic])
        .expect("Kafka Consumer should subscribe to kafka-topic");

    debug!("Begin Listening For Messages");
    loop {
        match consumer.recv().await {
            Ok(message) => {
                let span = trace_span!("Trace Source Message");
                message
                    .headers()
                    .conditional_extract_to_span(tracer.is_some(), &span);
                let _guard = span.enter();

                match message.payload() {
                    Some(payload) => {
                        if digitizer_analog_trace_message_buffer_has_identifier(payload) {
                            match root_as_digitizer_analog_trace_message(payload) {
                                Ok(message) => {
                                    info!(
                                        "Trace packet: dig. ID: {}, metadata: {:?}",
                                        message.digitizer_id(),
                                        message.metadata()
                                    );
                                    if let Err(e) = tdengine.process_message(&message).await {
                                        warn!("Error processing message : {e}");
                                    }
                                    if let Err(e) = tdengine.post_message().await {
                                        warn!("Error posting message to tdengine : {e}");
                                    }
                                }
                                Err(e) => warn!("Failed to parse message: {0}", e),
                            }
                        } else {
                            warn!("Message payload missing identifier.")
                        }
                    }
                    None => warn!("Error extracting payload from message."),
                };
                consumer
                    .commit_message(&message, CommitMode::Async)
                    .unwrap();
            }
            Err(e) => warn!("Error recieving message from server: {e}"),
        }
    }
}
