#[cfg(feature = "gpio")]
use byteorder::ByteOrder;
#[cfg(feature = "gpio")]
use drv_rp2040_gpio_api::Gpio;
use hif::*;

use hubris_num_tasks::Task;
use ringbuf::{ringbuf, ringbuf_entry};
#[cfg(feature = "gpio")]
use userlib::{FromPrimitive, task_slot};

#[cfg(feature = "gpio")]
task_slot!(GPIO, gpio_driver);

pub struct Buffer(#[allow(dead_code)] u8);

#[cfg(feature = "gpio")]
const fn bad_param(idx: u8) -> Failure {
    Failure::Fault(Fault::BadParameter(idx))
}

#[derive(Copy, Clone, Debug)]
#[cfg(feature = "gpio")]
struct Args<'a>(&'a [Option<u32>]);

#[cfg(feature = "gpio")]
impl<'a> Args<'a> {
    /// construct `Args` from `count` elements on the top of the stack
    fn new(stack: &'a [Option<u32>], count: usize) -> Result<Self, Failure> {
        if stack.len() < count {
            return Err(Failure::Fault(Fault::MissingParameters));
        }

        Ok(Self(&stack[stack.len() - count..]))
    }

    fn get(self, idx: u8) -> Result<u32, Failure> {
        self.0[idx as usize].ok_or(Failure::Fault(Fault::EmptyParameter(idx)))
    }

    fn get_pin(self, idx: u8) -> Result<Gpio, Failure> {
        Gpio::from_u32(self.get(idx)?).ok_or(bad_param(idx))
    }
}

#[cfg(feature = "gpio")]
fn gpio_input(
    stack: &[Option<u32>],
    _data: &[u8],
    rval: &mut [u8],
) -> Result<usize, Failure> {
    let gpio = drv_rp2040_gpio_api::Pins::from(GPIO.get_task_id());

    let args = Args::new(stack, 1)?;

    let pin = args.get_pin(0)?;

    let input = gpio.read_pin(pin);

    byteorder::LittleEndian::write_u16(rval, input as u16);

    Ok(core::mem::size_of::<u16>())
}

#[cfg(feature = "gpio")]
fn gpio_toggle(
    stack: &[Option<u32>],
    _data: &[u8],
    _rval: &mut [u8],
) -> Result<usize, Failure> {
    let gpio = drv_rp2040_gpio_api::Pins::from(GPIO.get_task_id());

    let args = Args::new(stack, 1)?;

    gpio.toggle_pin(args.get_pin(0)?);

    Ok(0)
}

#[cfg(feature = "gpio")]
fn gpio_set(
    stack: &[Option<u32>],
    _data: &[u8],
    _rval: &mut [u8],
) -> Result<usize, Failure> {
    let gpio = drv_rp2040_gpio_api::Pins::from(GPIO.get_task_id());

    let args = Args::new(stack, 1)?;

    gpio.set_pin(args.get_pin(0)?);

    Ok(0)
}

#[cfg(feature = "gpio")]
fn gpio_reset(
    stack: &[Option<u32>],
    _data: &[u8],
    _rval: &mut [u8],
) -> Result<usize, Failure> {
    let gpio = drv_rp2040_gpio_api::Pins::from(GPIO.get_task_id());

    let args = Args::new(stack, 1)?;

    gpio.clear_pin(args.get_pin(0)?);

    Ok(0)
}

#[cfg(feature = "gpio")]
fn gpio_direction(
    stack: &[Option<u32>],
    _data: &[u8],
    _rval: &mut [u8],
) -> Result<usize, Failure> {
    let gpio = drv_rp2040_gpio_api::Pins::from(GPIO.get_task_id());

    let args = Args::new(stack, 2)?;
    let pin = args.get_pin(0)?;
    let dir = drv_rp2040_gpio_api::Direction::from_u32(args.get(1)?)
        .ok_or(bad_param(1))?;

    gpio.set_dir(pin, dir);

    Ok(0)
}

pub(crate) static HIFFY_FUNCS: &[Function] = &[
    crate::common::sleep,
    crate::common::send,
    crate::common::send_lease_read,
    crate::common::send_lease_read_write,
    crate::common::send_lease_write,
    #[cfg(feature = "gpio")]
    gpio_input,
    #[cfg(feature = "gpio")]
    gpio_toggle,
    #[cfg(feature = "gpio")]
    gpio_set,
    #[cfg(feature = "gpio")]
    gpio_reset,
    #[cfg(feature = "gpio")]
    gpio_direction,
];

// the order of this enum must match the order in the array
pub enum Functions {
    Sleep(u16, u32),
    Send((Task, u16, Buffer, usize), u32),
    SendLeaseRead((Task, u16, Buffer, usize, usize), u32),
    SendLeaseReadWrite((Task, u16, Buffer, usize, usize, usize), u32),
    SendLeaseWrite((Task, u16, Buffer, usize, usize), u32),
    #[cfg(feature = "gpio")]
    GpioInput(Gpio, u32),
    #[cfg(feature = "gpio")]
    GpioToggle(Gpio, u32),
    #[cfg(feature = "gpio")]
    GpioSet(Gpio, u32),
    #[cfg(feature = "gpio")]
    GpioReset(Gpio, u32),
    #[cfg(feature = "gpio")]
    GpioDirection((Gpio, drv_rp2040_gpio_api::Direction), u32),
}

#[unsafe(no_mangle)]
pub static HIFFY_FUNCTIONS: Option<&Functions> = None;

#[derive(Debug, Copy, Clone, PartialEq)]
enum Trace {
    None,
    Execute((usize, hif::Op)),
    Failure(Failure),
    Success,
}

ringbuf!(Trace, 64, Trace::None);

pub(crate) fn trace_execute(offset: usize, op: hif::Op) {
    ringbuf_entry!(Trace::Execute((offset, op)));
}

pub(crate) fn trace_success() {
    ringbuf_entry!(Trace::Success);
}

pub(crate) fn trace_failure(f: hif::Failure) {
    ringbuf_entry!(Trace::Failure(f));
}
