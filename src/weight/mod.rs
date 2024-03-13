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

/// The timeout to wait until a new write / read is issued.
const IO_TIMEOUT: Duration = Duration::from_millis(1000);

/// The timeout to wait until a new port access is issued.
const PORT_TIMEOUT: Duration = Duration::from_secs(10);

type GuardPair = Arc<(Mutex<bool>, Condvar)>;
pub type WeightResult = Result<f64, Error>;

pub struct Scales {
    runloop_handle: Option<thread::JoinHandle<Result<(), AwakeError>>>,
    guard_pair: GuardPair,
    weight: Arc<Mutex<WeightResult>>,
}

impl Scales {
    pub fn on_serial_port(port_path: &str) -> Self {
        let guard_pair = Arc::new((Mutex::new(false), Condvar::new()));
        let guard_pair2 = Arc::clone(&guard_pair);

        let weight = Arc::new(Mutex::new(Err(Error::NotOpenedYet)));
        let weight2 = Arc::clone(&weight);

        let port_path = String::from(port_path);
        let runloop_handle = thread::spawn(|| Self::runloop(port_path, guard_pair2, weight2));

        Self {
            runloop_handle: Some(runloop_handle),
            guard_pair,
            weight,
        }
    }

    pub fn emulated() -> Self {
        let guard_pair = Arc::new((Mutex::new(false), Condvar::new()));
        let guard_pair2 = Arc::clone(&guard_pair);

        let weight = Arc::new(Mutex::new(Err(Error::NotOpenedYet)));
        let weight2 = Arc::clone(&weight);

        let runloop_handle = thread::spawn(move || Self::runloop_emulated(&guard_pair2.0, weight2));

        Self {
            runloop_handle: Some(runloop_handle),
            guard_pair,
            weight,
        }
    }

    pub fn weight(&self) -> WeightResult {
        self.weight.lock().unwrap().clone()
    }

    fn runloop(
        port_path: String,
        guard_pair: GuardPair,
        weight: Arc<Mutex<WeightResult>>,
    ) -> Result<(), AwakeError> {
        let (guard, cvar) = &*guard_pair;
        let weight = &*weight;

        loop {
            // Try to open the port.
            let port = Self::open_port(&port_path, guard, cvar, weight)?;

            // Yay, we have an open port.
            // Try to perform IO with it.
            Self::perform_io(port, guard, weight)?;

            // When we leave `perform_io()` without an `AwakeError`, the port has been lost.
            // Therefore, we simply restart the loop.
        }
    }

    fn open_port(
        port_path: &str,
        guard: &Mutex<bool>,
        cvar: &Condvar,
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

            // Wait the given timeout on the condvar.
            let (_, wait_result) = cvar
                .wait_timeout_while(guard.lock().unwrap(), PORT_TIMEOUT, |&mut should_exit| {
                    !should_exit
                })
                .unwrap();

            // If no timeout has happened, we have been awoken.
            // In that case, we leave the runloop.
            if !wait_result.timed_out() {
                return Err(AwakeError);
            }

            // Otherwise, we restart the loop and try to open the port again.
        }
    }

    fn perform_io(
        mut port: Box<dyn SerialPort>,
        guard: &Mutex<bool>,
        weight: &Mutex<WeightResult>,
    ) -> Result<(), AwakeError> {
        // Use this closure to test if we should awake from the runloop.
        let awake = || {
            if *guard.lock().unwrap() {
                Err(AwakeError)
            } else {
                Ok(())
            }
        };

        loop {
            // Send the info request.
            if let Err(err) = port.write_all(&[0x04, 0x05]) {
                *weight.lock().unwrap() = Err(err.into());
                return Ok(());
            }

            awake()?;

            // Read the result.
            let mut info_response = [0x00u8; 1];

            if let Err(err) = port.read_exact(&mut info_response) {
                *weight.lock().unwrap() = Err(err.into());
                return Ok(());
            }

            awake()?;

            // Send the weight request.
            if let Err(err) = port.write_all(&[0x13]) {
                *weight.lock().unwrap() = Err(err.into());
                return Ok(());
            }

            awake()?;

            // Read the result.
            let mut weight_response = [0x00u8; 45];

            if let Err(err) = port.read_exact(&mut weight_response) {
                *weight.lock().unwrap() = Err(err.into());
                return Ok(());
            }

            awake()?;

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
        }
    }

    fn runloop_emulated(
        guard: &Mutex<bool>,
        weight: Arc<Mutex<WeightResult>>,
    ) -> Result<(), AwakeError> {
        let mut fake_weight = 42.0;

        loop {
            // Fake a value.
            *weight.lock().unwrap() = Ok(fake_weight);

            // Sleep (or break out of the loop).
            for _ in 0..100 {
                thread::sleep(Duration::from_millis(10));

                if *guard.lock().unwrap() {
                    return Err(AwakeError);
                }
            }

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
        // Set the exit flag and issue the condvar.
        let (guard, cvar) = &*self.guard_pair;
        let mut should_exit = guard.lock().unwrap();

        *should_exit = true;
        drop(should_exit);

        cvar.notify_one();

        // Wait for the runloop to come down.
        _ = self.runloop_handle.take().unwrap().join().unwrap();
    }
}
