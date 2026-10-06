use packed_struct::prelude::*;
use std::error::Error;

use futures::stream::SplitSink;
use tokio::{
    net::TcpStream,
    sync::mpsc::{self, Receiver},
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::{
    drivers::unified_gamepad::{
        event::Event,
        reports::{
            input_capability_report::InputCapabilityReport, input_data_report::InputDataReport,
            ReportType, UNIFIED_SPEC_VERSION_MAJOR, UNIFIED_SPEC_VERSION_MINOR,
        },
    },
    input::{
        capability::Capability,
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    network::websocket::{SplitStream, WebsocketClient},
};

/// [WebsocketDevice] is a [SourceInputDevice] implementation that reads input
/// events over a websocket stream.
#[derive(Debug)]
pub struct WebsocketDevice {
    capabilities: Option<InputCapabilityReport>,
    state: Option<InputDataReport>,
    stream_read: Option<Receiver<Message>>,
    stream_write: Option<SplitSink<WebSocketStream<TcpStream>, Message>>,
    open_rx: Option<Receiver<SplitStream>>,
}

impl WebsocketDevice {
    pub fn new(client: WebsocketClient) -> Result<Self, Box<dyn Error + Send + Sync>> {
        log::info!("Starting websocket source device");
        let (open_tx, open_rx) = mpsc::channel(2);
        let client_clone = client.clone();
        tokio::task::spawn(async move {
            log::debug!("Opening websocket stream");
            let Some(stream) = client_clone.open().await else {
                log::error!("Websocket stream was already opened");
                return;
            };
            if let Err(e) = open_tx.send(stream).await {
                log::error!("Failed to send opened stream to running source device: {e}");
            }
        });

        Ok(Self {
            capabilities: None,
            state: None,
            stream_read: None,
            stream_write: None,
            open_rx: Some(open_rx),
        })
    }

    fn handle_report(
        &mut self,
        buf: &[u8],
        _bytes_read: usize,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        if buf.len() < 3 {
            log::debug!("Invalid report data");
            return Ok(vec![]);
        }

        // The first and second bytes contain the specification version
        let major_ver = buf[0];
        let minor_ver = buf[1];

        // The major version indicates there are breaking changes.
        if major_ver != UNIFIED_SPEC_VERSION_MAJOR {
            return Err(format!("Device major version (v{major_ver}) is not compatible with this implementation (v{UNIFIED_SPEC_VERSION_MAJOR})").into());
        }
        // Minor versions are backwards compatible
        if minor_ver > UNIFIED_SPEC_VERSION_MINOR {
            return Err(format!("Device minor version (v{minor_ver}) is newer than this implementation supports (v{UNIFIED_SPEC_VERSION_MINOR})").into());
        }

        let report_type = ReportType::from(buf[2]);
        log::trace!("Received report: {report_type:?}");
        match report_type {
            ReportType::Unknown => (),
            ReportType::InputCapabilityReport => {
                let report = InputCapabilityReport::unpack(buf)?;
                self.capabilities = Some(report);
                // If the capabilities change, zero out the old state
                self.state = None;
            }
            ReportType::InputDataReport => {
                return self.handle_input_report(buf);
            }
            ReportType::OutputCapabilityReport => (),
            ReportType::OutputDataReport => (),
        }

        Ok(vec![])
    }

    fn handle_input_report(
        &mut self,
        buf: &[u8],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let buffer = buf.try_into()?;
        let report = InputDataReport::unpack(buffer)?;
        let old_state = self.state.take();
        self.state = Some(report);

        let Some(old_state) = old_state.as_ref() else {
            return Ok(vec![]);
        };
        let Some(state) = self.state.as_ref() else {
            return Ok(vec![]);
        };
        if state.state_version == old_state.state_version {
            return Ok(vec![]);
        }
        let Some(capabilities) = self.capabilities.as_ref() else {
            return Ok(vec![]);
        };

        let old_values = capabilities.decode_data_report(old_state)?;
        let values = capabilities.decode_data_report(state)?;
        let values_iter = old_values.iter().zip(values.iter());

        let mut events = Vec::new();
        for (info, (old_value, value)) in capabilities.get_capabilities().iter().zip(values_iter) {
            if old_value == value {
                continue;
            }
            let capability = info.capability;
            let event = Event {
                capability,
                value: value.to_owned(),
            };
            events.push(event);
        }

        Ok(events)
    }
}

impl SourceInputDevice for WebsocketDevice {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        // Open the websocket stream if it's not already
        if let Some(mut open_rx) = self.open_rx.take() {
            if open_rx.is_closed() && open_rx.is_empty() {
                log::error!("Channel to open stream is already closed");
                return Err(InputError::DeviceError("Failed to open stream".into()));
            }
            if open_rx.is_empty() {
                self.open_rx = Some(open_rx);
                return Ok(vec![]);
            }
            let Some(stream) = open_rx.blocking_recv() else {
                log::error!("No stream found after opening");
                return Ok(vec![]);
            };
            let (writer, reader) = stream;
            self.stream_read = Some(reader);
            self.stream_write = Some(writer);
            log::debug!("Successfully opened websocket stream");
        }

        // Read all messages from the stream
        let msgs = {
            let Some(reader) = self.stream_read.as_mut() else {
                log::debug!("No stream receiver exists!");
                return Ok(vec![]);
            };
            if reader.is_closed() {
                log::debug!("Stream is closed!");
                return Err(InputError::DeviceError("Stream closed".into()));
            }
            if reader.is_empty() {
                return Ok(vec![]);
            }

            // Read messages from the socket
            let mut messages = vec![];
            while !(reader.is_empty()) {
                let Some(msg) = reader.blocking_recv() else {
                    break;
                };
                messages.push(msg);
            }
            messages
        };

        // Handle each message as an HID report
        let mut events = Vec::with_capacity(msgs.len());
        for msg in msgs {
            let data = msg.into_data();
            let websocket_events = self
                .handle_report(&data, data.len())?
                .into_iter()
                .map(|e| e.into());
            events.extend(websocket_events);
        }
        log::trace!("Got events: {events:?}");

        Ok(events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }
}

impl SourceOutputDevice for WebsocketDevice {}
