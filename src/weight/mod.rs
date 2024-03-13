use std::fmt::Display;
use std::io::Error as IOError;
use std::str;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use serialport::{DataBits, Error as SerialPortError, FlowControl, Parity, SerialPort, StopBits};

#[derive(Debug, Clone)]
pub enum Error {
    NotOpenedYet,
    SerialPort(SerialPortError),
    IO(String),
    FailedToParse,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            NotOpenedYet => write!(f, "The serial port has not been opened yet."),
            SerialPort(err) => write!(f, "{}", err),
            IO(err) => write!(f, "{}", err),
            FailedToParse => write!(f, "Failed to parse response."),
        }
    }
}

impl std::error::Error for Error {}

impl From<SerialPortError> for Error {
    fn from(value: SerialPortError) -> Self {
        Error::SerialPort(value)
    }
}

impl From<IOError> for Error {
    fn from(value: IOError) -> Self {
        Error::IO(value.to_string())
    }
}

#[derive(Debug, Copy, Clone)]
struct AwakeError;

impl Display for AwakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "The runloop has been awoken.")
    }
}

impl std::error::Error for AwakeError {}

/// A guard to notify the runloop that it should exit
struct Guard(Mutex<bool>, Condvar);

impl Guard {
    /// Block on the guard until it is cancelled or `timeout_duration` runs out.
    /// In case of cancel, `Err(AwakeError)` is returned.
    fn wait(&self, timeout_duration: Duration) -> Result<(), AwakeError> {
        // Block up to `timeout_duration` on the cvar.
        let (_, wait_result) = self
            .1
            .wait_timeout_while(
                self.0.lock().unwrap(),
                timeout_duration,
                |&mut should_exit| !should_exit,
            )
            .unwrap();

        // If no timeout has happened, we have been awoken.
        // In that case, we leave the runloop.
        if wait_result.timed_out() {
            Ok(())
        } else {
            Err(AwakeError)
        }
    }

    /// Like `wait()`, but just check the guard without blocking.
    fn check(&self) -> Result<(), AwakeError> {
        if *self.0.lock().unwrap() {
            Err(AwakeError)
        } else {
            Ok(())
        }
    }

    fn cancel(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_one();
    }
}

impl Default for Guard {
    fn default() -> Self {
        Self(Mutex::new(false), Condvar::new())
    }
}

/// The timeout to wait until a new write / read is issued.
const IO_TIMEOUT: Duration = Duration::from_millis(1000);

/// The timeout to wait until a new port access is issued.
const PORT_TIMEOUT: Duration = Duration::from_secs(10);

/// The result of a weight poll
pub type WeightResult = Result<f64, Error>;

pub struct Scales {
    runloop_handle: Option<thread::JoinHandle<Result<(), AwakeError>>>,
    guard: Arc<Guard>,
    weight: Arc<Mutex<WeightResult>>,
}

impl Scales {
    pub fn on_serial_port(port_path: &str) -> Self {
        let guard = Arc::new(Guard::default());
        let guard2 = Arc::clone(&guard);

        let weight = Arc::new(Mutex::new(Err(Error::NotOpenedYet)));
        let weight2 = Arc::clone(&weight);

        let port_path = String::from(port_path);
        let runloop_handle = thread::spawn(move || Self::runloop(port_path, &guard2, &weight2));

        Self {
            runloop_handle: Some(runloop_handle),
            guard,
            weight,
        }
    }

    pub fn emulated() -> Self {
        let guard = Arc::new(Guard::default());
        let guard2 = Arc::clone(&guard);

        let weight = Arc::new(Mutex::new(Err(Error::NotOpenedYet)));
        let weight2 = Arc::clone(&weight);

        let runloop_handle = thread::spawn(move || Self::runloop_emulated(&guard2, &weight2));

        Self {
            runloop_handle: Some(runloop_handle),
            guard,
            weight,
        }
    }

    pub fn weight(&self) -> WeightResult {
        self.weight.lock().unwrap().clone()
    }

    fn runloop(
        port_path: String,
        guard: &Guard,
        weight: &Mutex<WeightResult>,
    ) -> Result<(), AwakeError> {
        loop {
            // Try to open the port.
            let port = Self::open_port(&port_path, guard, weight)?;

            // Yay, we have an open port.
            // Try to perform IO with it.
            Self::perform_io(port, guard, weight)?;

            // When we leave `perform_io()` without an `AwakeError`, the port has been lost.
            // Therefore, we simply restart the loop.
        }
    }

    fn open_port(
        port_path: &str,
        guard: &Guard,
        weight: &Mutex<WeightResult>,
    ) -> Result<Box<dyn SerialPort>, AwakeError> {
        loop {
            // Specify the characteristics of the port.
            let port_builder = serialport::new(port_path, 9600)
                .data_bits(DataBits::Eight)
                .stop_bits(StopBits::One)
                .flow_control(FlowControl::None)
                .parity(Parity::None)
                .timeout(IO_TIMEOUT);

            // Try to open it. If that succeeds, we return instantly.
            // Errors are recorded in the weight mutex.
            match port_builder.open() {
                Ok(port) => return Ok(port),
                Err(err) => *weight.lock().unwrap() = Err(err.into()),
            }

            // Wait the given timeout on the guard.
            // If it fires, we have been cancelled and leave the runloop.
            guard.wait(PORT_TIMEOUT)?;

            // Otherwise, we restart the loop and try to open the port again.
        }
    }

    fn perform_io(
        mut port: Box<dyn SerialPort>,
        guard: &Guard,
        weight: &Mutex<WeightResult>,
    ) -> Result<(), AwakeError> {
        loop {
            // Send the info request.
            if let Err(err) = port.write_all(&[0x04, 0x05]) {
                *weight.lock().unwrap() = Err(err.into());
                return Ok(());
            }

            guard.check()?;

            // Read the result.
            let mut info_response = [0x00u8; 1];

            if let Err(err) = port.read_exact(&mut info_response) {
                *weight.lock().unwrap() = Err(err.into());
                return Ok(());
            }

            guard.check()?;

            // Send the weight request.
            if let Err(err) = port.write_all(&[0x13]) {
                *weight.lock().unwrap() = Err(err.into());
                return Ok(());
            }

            guard.check()?;

            // Read the result.
            let mut weight_response = [0x00u8; 45];

            if let Err(err) = port.read_exact(&mut weight_response) {
                *weight.lock().unwrap() = Err(err.into());
                return Ok(());
            }

            guard.check()?;

            // Extract the sign.
            let sign = match weight_response[14] {
                0x20 => 1.0,
                0x2d => -1.0,

                _ => {
                    *weight.lock().unwrap() = Err(Error::FailedToParse);
                    return Ok(());
                }
            };

            // Extract the digits.
            let weight_bytes = &weight_response[15..21];

            let weight_str = match str::from_utf8(weight_bytes) {
                Ok(weight_str) => weight_str,

                Err(_) => {
                    *weight.lock().unwrap() = Err(Error::FailedToParse);
                    return Ok(());
                }
            };

            // Parse the string slice.
            match weight_str.trim().parse::<f64>() {
                Ok(weight_kg) => *weight.lock().unwrap() = Ok(sign * weight_kg),

                Err(_) => {
                    *weight.lock().unwrap() = Err(Error::FailedToParse);
                    return Ok(());
                }
            }

            // Just to prevent a busy loop ... probably unnecessary
            // because the serial port induces blocking ...
            guard.wait(Duration::from_millis(1))?;
        }
    }

    fn runloop_emulated(guard: &Guard, weight: &Mutex<WeightResult>) -> Result<(), AwakeError> {
        let mut fake_weight = 42.0;

        loop {
            // Fake a value.
            *weight.lock().unwrap() = Ok(fake_weight);

            // Wait a second on the guard.
            guard.wait(Duration::from_secs(1))?;

            // Vary the value.
            if fake_weight > 50.0 {
                fake_weight = 42.0;
            } else {
                fake_weight += 0.1;
            }
        }
    }
}

impl Drop for Scales {
    fn drop(&mut self) {
        // Cancel the guard and wait for the runloop to come down.
        self.guard.cancel();
        _ = self.runloop_handle.take().unwrap().join().unwrap();
    }
}
