//! Declared host services consumed by the web shell.

use std::{collections::BTreeMap, io, sync::Mutex, time::Duration};

use sim_transport_ports::TransportServices;

/// Platform services required by the host-neutral shell.
pub trait ShellServices: Send + Sync {
    /// Transport realization used to listen for HTTP requests.
    fn transport(&self) -> TransportServices;
    /// Read one file through an explicitly mounted shell namespace.
    fn read_mount(&self, path: &str) -> io::Result<Vec<u8>>;
    /// Monotonic model time used for session expiry and ordering.
    fn monotonic(&self) -> Duration;
    /// Fill opaque session identity bytes from the capsule entropy service.
    fn fill_entropy(&self, bytes: &mut [u8]) -> io::Result<()>;
    /// Ask the capsule to open an external URL. Headless capsules may refuse.
    fn open_external(&self, url: &str) -> io::Result<()>;
}

/// Deterministic shell services for tests and modeled hosts.
pub struct ModelShellServices {
    transport: TransportServices,
    files: BTreeMap<String, Vec<u8>>,
    time: Mutex<Duration>,
    entropy: Mutex<u128>,
    opened: Mutex<Vec<String>>,
}

impl ModelShellServices {
    /// Construct a deterministic service set over modeled transport and mounts.
    pub fn new(transport: TransportServices, files: BTreeMap<String, Vec<u8>>) -> Self {
        Self {
            transport,
            files,
            time: Mutex::new(Duration::ZERO),
            entropy: Mutex::new(1),
            opened: Mutex::new(Vec::new()),
        }
    }

    /// Set the model clock explicitly.
    pub fn set_time(&self, time: Duration) {
        *self.time.lock().expect("model clock lock") = time;
    }

    /// Return URLs requested through the modeled external-open service.
    pub fn opened(&self) -> Vec<String> {
        self.opened.lock().expect("model open lock").clone()
    }
}

impl ShellServices for ModelShellServices {
    fn transport(&self) -> TransportServices {
        self.transport.clone()
    }

    fn read_mount(&self, path: &str) -> io::Result<Vec<u8>> {
        self.files.get(path).cloned().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("unmounted shell file: {path}"),
            )
        })
    }

    fn monotonic(&self) -> Duration {
        *self.time.lock().expect("model clock lock")
    }

    fn fill_entropy(&self, bytes: &mut [u8]) -> io::Result<()> {
        let mut value = self.entropy.lock().expect("model entropy lock");
        let seed = value.to_le_bytes();
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = seed[index % seed.len()];
        }
        *value = value.saturating_add(1);
        Ok(())
    }

    fn open_external(&self, url: &str) -> io::Result<()> {
        self.opened
            .lock()
            .expect("model open lock")
            .push(url.to_owned());
        Ok(())
    }
}
