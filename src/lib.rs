//! Ebyte module control.

#![doc = include_str!("../README.md")]

mod adapter;

use crate::adapter::Serial;
use adapter::{CtsAux, M0Dtr, M1Rts, StandardDelay};
use anyhow::Context;
use ebyte_e32::{mode::Normal, Ebyte};
use embedded_hal::{
    blocking::delay,
    digital::v2::{InputPin, OutputPin},
    serial::{self, Read, Write},
};
use serial_core::{BaudRate, CharSize, FlowControl, PortSettings, SerialPort, StopBits};

//     gpio_cdev::{Chip, LineRequestFlags},
//     serial_core::{BaudRate, CharSize, FlowControl, PortSettings, SerialPort},
//     serial_unix::TTYPort,
//     CdevPin as Pin, {Delay, Serial},
// };
use nb::block;
use rustyline::{error::ReadlineError, Editor};
use std::{cell::RefCell, fmt::Debug, rc::Rc, time::Duration};

use arguments::{Args, Mode};
use config::load;

/// Configuration from `Config.toml`.
pub mod config;

/// Command line interface.
pub mod arguments;

/// Setup the hardware, then load some parameters,
/// update them if needed, then listen, send, or read model data.
///
/// # Panics
/// Failed initialization of the module driver
/// or communicating with the module may cause a panic.
pub fn create(
    args: &Args,
) -> anyhow::Result<Ebyte<Serial, CtsAux, M0Dtr, M1Rts, StandardDelay, Normal>> {
    let config = load(&args.config).context("Failed to get config")?;
    let baud_rate = BaudRate::from_speed(config.baudrate as usize);
    let stop_bits = if config.stop_bits == 1 {
        StopBits::Stop1
    } else if config.stop_bits == 2 {
        StopBits::Stop2
    } else {
        panic!()
    };

    let settings: PortSettings = PortSettings {
        baud_rate,
        char_size: CharSize::Bits8,
        parity: config.parity.into(),
        stop_bits,
        flow_control: FlowControl::FlowNone,
    };

    let mut port = ::serial::open(&config.serial_path)
        .with_context(|| format!("Failed to open TTY {}", config.serial_path.display()))?;
    port.set_timeout(Duration::from_secs(1000))?;
    port.configure(&settings)
        .context("Failed to set up serial port")?;

    let port = Rc::new(RefCell::new(port));
    let serial = Serial::new(port.clone());

    let aux = CtsAux::new(port.clone());

    let m0 = M0Dtr::new(port.clone());
    let m1 = M1Rts::new(port.clone());
    let delay = StandardDelay {};
    Ebyte::new(serial, aux, m0, m1, delay).context("Failed to initialize driver")
}

pub fn run<S, AUX, M0, M1, D>(
    args: &Args,
    mut ebyte: Ebyte<S, AUX, M0, M1, D, Normal>,
) -> anyhow::Result<()>
where
    S: serial::Read<u8> + serial::Write<u8>,
    <S as serial::Read<u8>>::Error: Debug,
    <S as serial::Write<u8>>::Error: Debug,
    AUX: InputPin,
    M0: OutputPin,
    M1: OutputPin,
    D: delay::DelayMs<u32>,
{
    match args.mode {
        Mode::Send => send(ebyte),
        Mode::Listen => loop {
            let b = block!(ebyte.read()).expect("Failed to read");
            print!("{}", b as char);
            std::io::Write::flush(&mut std::io::stdout()).context("Failed to flush")?;
        },
        Mode::ReadModelData => {
            println!("Reading model data");
            let model_data = ebyte.model_data().context("Failed to read model data")?;
            println!("{model_data:#?}");
            Ok(())
        }
        Mode::ReadParameters => {
            println!("Reading parameter data");
            let parameters = ebyte
                .parameters()
                .context("Failed to read parameter data")?;
            println!("{parameters:#?}");
            Ok(())
        }
        Mode::Configure(ref parameters) => configure(ebyte, parameters),
    }
}

fn send<S, AUX, M0, M1, D>(
    mut ebyte: Ebyte<S, AUX, M0, M1, D, ebyte_e32::mode::Normal>,
) -> anyhow::Result<()>
where
    S: serial::Read<u8> + serial::Write<u8>,
    <S as serial::Read<u8>>::Error: Debug,
    <S as serial::Write<u8>>::Error: Debug,
    AUX: InputPin,
    M0: OutputPin,
    M1: OutputPin,
    D: delay::DelayMs<u32>,
{
    let mut prompt = Editor::<()>::new().context("Failed to set up prompt")?;
    loop {
        match prompt.readline("Enter message >> ") {
            Ok(line) => {
                if line == "exit" || line == "quit" {
                    break;
                }
                prompt.add_history_entry(&line);

                for b in line.as_bytes() {
                    block!(ebyte.write(*b)).expect("Failed to write");
                    print!("{}", *b as char);
                    std::io::Write::flush(&mut std::io::stdout()).context("Failed to flush")?;
                }
                block!(ebyte.write(b'\n')).expect("Failed to write");
                println!();
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {err:?}");
                break;
            }
        }
    }
    Ok(())
}

/// Apply new parameters to the Ebyte module, optionally persisting them.
/// 1. Read old parameters from module.
/// 2. Check if the given parameters and the loaded ones are equal, if so, bail.
/// 3. Apply new parameters.
/// 4. Read them back, warning if they are not equal to the given parameters.
fn configure<S, AUX, M0, M1, D>(
    mut ebyte: Ebyte<S, AUX, M0, M1, D, ebyte_e32::mode::Normal>,
    parameters: &arguments::Parameters,
) -> anyhow::Result<()>
where
    S: serial::Read<u8> + serial::Write<u8>,
    <S as serial::Read<u8>>::Error: Debug,
    <S as serial::Write<u8>>::Error: Debug,
    AUX: InputPin,
    M0: OutputPin,
    M1: OutputPin,
    D: delay::DelayMs<u32>,
{
    println!("Loading existing parameters");
    let old_params = ebyte
        .parameters()
        .context("Failed to read existing parameters")?;
    println!("Loaded parameters: {old_params:#?}");

    // Create Ebyte parameters from argument parameters.
    let new_params = ebyte_e32::Parameters::from(parameters);

    if new_params == old_params {
        println!("Leaving parameters unchanged");
    } else {
        println!(
            "Updating parameters (persistence: {:?})",
            parameters.persistence
        );
        ebyte
            .set_parameters(&new_params, parameters.persistence)
            .context("Failed to set new parameters")?;

        // Check if it worked.
        let current_params = ebyte
            .parameters()
            .context("Failed to read current parameters")?;
        if current_params == new_params {
            println!("Successfully applied new parameters");
        } else {
            eprintln!("Error: parameters unchanged: {current_params:#?}");
        }
    }
    Ok(())
}
