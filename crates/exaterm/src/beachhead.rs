use crate::remote::{connect_remote, RemoteBeachheadBridge};
use exaterm_core::daemon::LocalBeachheadClient;
use exaterm_core::proto::{ClientMessage, ServerMessage};
use std::os::unix::net::UnixStream;
use std::sync::{mpsc, Arc, Mutex};

#[derive(Clone, Debug)]
pub enum BeachheadTarget {
    Local,
    Ssh(String),
}

pub struct BeachheadConnection {
    client: LocalBeachheadClient,
    _remote_bridge: Option<RemoteBeachheadBridge>,
}

impl BeachheadConnection {
    pub fn connect(target: &BeachheadTarget) -> Result<Self, String> {
        match target {
            BeachheadTarget::Local => Ok(Self {
                client: LocalBeachheadClient::connect_or_spawn()?,
                _remote_bridge: None,
            }),
            BeachheadTarget::Ssh(target) => {
                let (client, bridge) = connect_remote(target)?;
                Ok(Self {
                    client,
                    _remote_bridge: Some(bridge),
                })
            }
        }
    }

    pub fn commands(&self) -> &mpsc::Sender<ClientMessage> {
        &self.client.commands
    }

    pub fn events(&self) -> &mpsc::Receiver<ServerMessage> {
        &self.client.events
    }

    pub fn raw_writer(&self) -> Arc<Mutex<UnixStream>> {
        self.client.raw_writer.clone()
    }

    pub fn take_raw_reader(&self) -> Option<UnixStream> {
        self.client
            .raw_reader
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
    }
}
