use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Utc};
use metrics::counter;
use metrics_exporter_prometheus::PrometheusBuilder;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    Message, Timestamp,
};
use supermusr_common::metrics::{
    failures::{self, FailureKind},
    messages_received::{self, MessageKind},
    metric_names::{FAILURES, MESSAGES_RECEIVED},
};
use supermusr_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::{
        digitizer_analog_trace_message_buffer_has_identifier,
        root_as_digitizer_analog_trace_message,
    },
    ecs_6s4t_run_stop_generated::run_stop_buffer_has_identifier,
    ecs_pl72_run_start_generated::run_start_buffer_has_identifier,
};
use tracing::{debug, info, warn};

use crate::{file::TraceFile, ControlOpts};

pub(crate) async fn run(control_args: ControlOpts) -> Result<()> {
    let common_args = control_args.common;

    let consumer: StreamConsumer = supermusr_common::generate_kafka_client_config(
        &common_args.broker,
        &common_args.username,
        &common_args.password,
    )
    .set("group.id", &common_args.consumer_group)
    .set("enable.partition.eof", "false")
    .set("session.timeout.ms", "6000")
    .set("enable.auto.commit", "false")
    .create()?;

    // Install exporter and register metrics
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(common_args.observability_address)
        .install()
        .expect("Prometheus metrics exporter should be setup");

    metrics::describe_counter!(
        MESSAGES_RECEIVED,
        metrics::Unit::Count,
        "Number of messages received"
    );
    metrics::describe_counter!(
        FAILURES,
        metrics::Unit::Count,
        "Number of failures encountered"
    );

    let topics_to_subscribe = [
        control_args.control_topic.as_str(),
        common_args.trace_topic.as_str(),
    ];

    consumer.subscribe(&topics_to_subscribe)?;

    let mut file: Option<TraceFile> = None;

    loop {
        match consumer.recv().await {
            Err(e) => warn!("Kafka error: {}", e),
            Ok(msg) => {
                debug!(
                    "key: '{:?}', topic: {}, partition: {}, offset: {}, timestamp: {:?}",
                    msg.key(),
                    msg.topic(),
                    msg.partition(),
                    msg.offset(),
                    msg.timestamp()
                );

                if let Some(payload) = msg.payload() {
                    if digitizer_analog_trace_message_buffer_has_identifier(payload) {
                        // A message has been received from the trace topic.
                        match root_as_digitizer_analog_trace_message(payload) {
                            Ok(data) => {
                                info!(
                                    "Trace packet: dig. ID: {}, metadata: {:?}",
                                    data.digitizer_id(),
                                    data.metadata()
                                );
                                counter!(
                                    MESSAGES_RECEIVED,
                                    &[messages_received::get_label(MessageKind::Trace)]
                                )
                                .increment(1);

                                if let Some(ref mut file) = file {
                                    info!("Writing trace data to \"{}\"", file.filename());
                                    if let Err(e) = file.push(&data) {
                                        warn!("Failed to save traces to file: {}", e);
                                        counter!(
                                            FAILURES,
                                            &[failures::get_label(FailureKind::FileWriteFailed)]
                                        )
                                        .increment(1);
                                    }
                                }
                            }

                            Err(e) => {
                                warn!("Failed to parse message: {}", e);
                                counter!(
                                    FAILURES,
                                    &[failures::get_label(FailureKind::UnableToDecodeMessage)]
                                )
                                .increment(1);
                            }
                        }
                    } else if *msg.topic() == control_args.control_topic {
                        // A message has been received from the control topic.
                        if run_start_buffer_has_identifier(payload) {
                            debug!("New run start.");
                            // Start recording trace data to file.
                            if file.is_none() {
                                if let Ok(filename) = generate_filename(msg.timestamp()) {
                                    file = Some(TraceFile::create(
                                        &filename,
                                        common_args.digitizer_count,
                                    )?);
                                    debug!("Created new trace file: {:?}", filename);
                                } else {
                                    warn!("Failed to create new trace file.");
                                    counter!(
                                        FAILURES,
                                        &[failures::get_label(FailureKind::FileWriteFailed)]
                                    )
                                    .increment(1);
                                }
                            }
                            // If file already exists, do nothing.
                        } else if run_stop_buffer_has_identifier(payload) {
                            debug!("New run stop.");
                            // Stop recording trace data to file.
                            file = None;
                        } else {
                            warn!("Incorrect message identifier on topic \"{}\"", msg.topic());
                        }
                    } else {
                        // The message kind is unknown.
                        warn!("Unexpected message type on topic \"{}\"", msg.topic());
                        counter!(
                            MESSAGES_RECEIVED,
                            &[messages_received::get_label(MessageKind::Unknown)]
                        )
                        .increment(1);
                    }
                }

                consumer.commit_message(&msg, CommitMode::Async).unwrap();
            }
        };
    }
}

fn generate_filename(timestamp: Timestamp) -> Result<PathBuf> {
    //  TODO: Check this unwrap does not cause any issues.
    if let Some(timestamp) = timestamp.to_millis() {
        if let Some(timestamp) = DateTime::<Utc>::from_timestamp_millis(timestamp) {
            return Ok(PathBuf::from(format!("{:?}.h5", timestamp)));
        }
    }
    Err(anyhow::anyhow!(
        "Failed to convert timestamp to milliseconds"
    ))
}
