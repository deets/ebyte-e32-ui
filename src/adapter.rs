use std::{cell::RefCell, rc::Rc, thread, time::Duration};

use embedded_hal::{
    blocking::delay::DelayMs,
    digital::v2::{InputPin, OutputPin},
    serial::{Read, Write},
};
use serial_core::SerialPort;

type PortType = Rc<RefCell<dyn SerialPort>>;

pub struct Serial {
    port: PortType,
}
pub struct CtsAux {
    port: PortType,
}
pub struct M0Dtr {
    port: PortType,
}
pub struct M1Rts {
    port: PortType,
}
pub struct StandardDelay {}

impl Serial {
    pub fn new(port: PortType) -> Self {
        Self { port }
    }
}

impl CtsAux {
    pub fn new(port: PortType) -> Self {
        Self { port }
    }
}
impl M0Dtr {
    pub fn new(port: PortType) -> Self {
        Self { port }
    }
}
impl M1Rts {
    pub fn new(port: PortType) -> Self {
        Self { port }
    }
}

impl Read<u8> for Serial {
    type Error = std::io::Error;

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        let mut result = [0];
        self.port.borrow_mut().read(&mut result)?;
        Ok(result[0])
    }
}

impl Write<u8> for Serial {
    type Error = std::io::Error;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        let buf = [word];
        self.port.borrow_mut().write_all(&buf)?;
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.port.borrow_mut().flush()?;
        Ok(())
    }
}

impl InputPin for CtsAux {
    type Error = serial::Error;

    fn is_high(&self) -> Result<bool, Self::Error> {
        Ok(self.port.borrow_mut().read_cts()? == false)
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        Ok(!self.is_high()?)
    }
}

impl OutputPin for M0Dtr {
    type Error = serial::Error;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.port.borrow_mut().set_dtr(true)?;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.port.borrow_mut().set_dtr(false)?;
        Ok(())
    }
}

impl OutputPin for M1Rts {
    type Error = serial::Error;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.port.borrow_mut().set_rts(true)?;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.port.borrow_mut().set_rts(false)?;
        Ok(())
    }
}

impl DelayMs<u32> for StandardDelay {
    fn delay_ms(&mut self, ms: u32) {
        thread::sleep(Duration::from_millis(ms as u64));
    }
}
