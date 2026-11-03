// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! PIO Driver
//!
//!
//! Lifecycle of an SM:
//!
//! ```text
//!                  ╭───────╮           ┌───────────┐
//!                  │ Start │──────────>│ Create SM │
//!                  ╰───────╯           └───────────┘
//!                                             │
//!                                             │
//!                                             │
//!                                             🮦
//!        ╭────────────╮                 ╭────────────╮
//!        │            │<─────start<─────│            │             ╭─────╮
//!        │ RUNNING SM │                 │ STOPPED SM │───>remove──>│ End │
//!        │            │──────>stop─────>│            │             ╰─────╯
//!        ╰────────────╯                 ╰────────────╯
//!          │      🮧                        │      🮧
//!          🮦      │                        │      │
//!        ┌────┐ ┌────┐                     │      │
//!        │ RX │ │ TX │                     │      │
//!        └────┘ └────┘                     │      │
//!                                          🮦      │
//!                                       ┌──────────────┐
//!                                       │ Configure SM │
//!                                       └──────────────┘
//! ```

#![no_std]
#![no_main]

use core::{
    cell::OnceCell,
    ops::{Deref, Index},
};

use drv_pio_api::{EnumCount, PioError, PioInst, PioProgram, Sm};
use idol_runtime::{NotificationHandler, RequestError};
use rp2040_hal::{pac, reset_hw};

const INSTRUCTION_COUNT: usize = 32;

type Program = pio::Program<{ pio::RP2040_MAX_PROGRAM_SIZE }>;

#[derive(Debug, Default)]
struct Programs([OnceCell<Program>; PioProgram::COUNT]);

impl Index<PioProgram> for Programs {
    type Output = Program;

    fn index(&self, index: PioProgram) -> &Self::Output {
        self.0[index as usize].get_or_init(|| match index {
            PioProgram::PioBlink => {
                pio::pio_asm!(
                    ".wrap_target",
                    "set pins, 1 [31]",
                    "set pins, 0 [31]",
                    ".wrap"
                )
                .program
            }
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct InstalledProgram {
    #[allow(dead_code)]
    offset: u8,
    // Retained so a future remove_sm operation can release the allocation.
    #[allow(dead_code)]
    length: u8,
}

#[derive(Debug, Clone, Copy)]
struct StateMachine {
    // Retained so future state-machine operations can recover the allocation.
    #[allow(dead_code)]
    program: InstalledProgram,
}

#[derive(Debug)]
struct ServerImpl {
    programs: Programs,
    pio0: Pio<pac::PIO0>,
    pio1: Pio<pac::PIO1>,
}

#[derive(Debug)]
struct Pio<PacPio> {
    pio: PacPio,
    used_instruction_space: u32,
    state_machines: [Option<StateMachine>; 4],
}

trait PioBlock: Deref<Target = rp2040_hal::pac::pio0::RegisterBlock> {
    const INST: PioInst;
}

impl PioBlock for pac::PIO0 {
    const INST: PioInst = PioInst::Pio0;
}

impl PioBlock for pac::PIO1 {
    const INST: PioInst = PioInst::Pio1;
}

impl<PacPio: PioBlock> Pio<PacPio> {
    fn new(pio: PacPio) -> Self {
        Self {
            pio,
            used_instruction_space: 0,
            state_machines: [None; 4],
        }
    }

    fn offset(&self, program: &Program, mask: u32) -> Result<usize, PioError> {
        match program.origin {
            Some(origin)
                if origin as usize > INSTRUCTION_COUNT - program.code.len()
                    || self.used_instruction_space & (mask << origin) != 0 =>
            {
                Err(PioError::NoInstructionMemory)
            }
            Some(origin) => Ok(origin as usize),
            None => (0..=(INSTRUCTION_COUNT - program.code.len()))
                .rev()
                .find(|offset| {
                    self.used_instruction_space & (mask << offset) == 0
                })
                .ok_or(PioError::NoInstructionMemory),
        }
    }

    fn install(&mut self, sm: u8, program: &Program) -> Result<(), PioError> {
        if program.code.is_empty() || program.code.len() > INSTRUCTION_COUNT {
            return Err(PioError::InvalidProgram);
        }

        let length = program.code.len();
        let mask = if length == INSTRUCTION_COUNT {
            u32::MAX
        } else {
            (1u32 << length) - 1
        };
        let offset = self.offset(program, mask)? as u8;

        for (index, instruction) in program.code.iter().copied().enumerate() {
            const JMP_MASK: u16 = 0b1110_0000_0000_0000;
            const ADDR_MASK: u16 = 0b11111;
            // If it's a jump instruction, add the offset to instruction
            let instruction = if instruction & JMP_MASK == 0 {
                let address = (instruction & ADDR_MASK) as u8;
                let address = address
                    .checked_add(offset)
                    .ok_or(PioError::InvalidProgram)?;
                if address >= INSTRUCTION_COUNT as u8 {
                    return Err(PioError::InvalidProgram);
                }
                (instruction & !ADDR_MASK) | address as u16
            } else {
                instruction
            };

            self.pio
                .instr_mem(index + offset as usize)
                .write(|w| unsafe { w.instr_mem0().bits(instruction) });
        }

        self.used_instruction_space |= mask << offset;

        self.configure_stopped(sm, offset, program);

        self.state_machines[sm as usize] = Some(StateMachine {
            program: InstalledProgram {
                offset,
                length: length as u8,
            },
        });
        Ok(())
    }

    fn configure_stopped(&self, sm: u8, offset: u8, p: &Program) {
        let block = &*self.pio;
        let sm_registers = block.sm(sm as usize);
        let sm_mask = 1u32 << sm;

        // Safety: this server exclusively owns both PIO blocks and their state
        // machines after taking the peripherals in `new`.
        unsafe {
            block.ctrl().modify(|r, w| {
                w.sm_enable().bits(r.sm_enable().bits() & !(sm_mask as u8))
            });

            sm_registers.sm_execctrl().write(|w| {
                w.side_en().bit(p.side_set.optional());
                w.side_pindir().bit(p.side_set.pindirs());
                w.wrap_top().bits(offset + p.wrap.source);
                w.wrap_bottom().bits(offset + p.wrap.target)
            });

            sm_registers.sm_pinctrl().write(|w| {
                w.sideset_count().bits(p.side_set.bits());
                w.set_count().bits(5);
                w.out_count().bits(0);
                w.in_base().bits(0);
                w.sideset_base().bits(0);
                w.set_base().bits(0);
                w.out_base().bits(0)
            });

            block
                .ctrl()
                .modify(|_, w| w.sm_restart().bits(sm_mask as u8));
            block
                .ctrl()
                .modify(|_, w| w.clkdiv_restart().bits(sm_mask as u8));

            // Start execution at the installed program's offset.  The state
            // machine remains disabled until a future start operation enables it.
            sm_registers
                .sm_instr()
                .write(|w| w.sm0_instr().bits(offset as u16));
        }
    }

    fn free_state_machine(&self) -> Option<Sm> {
        self.state_machines
            .iter()
            .position(Option::is_none)
            .map(|idx| Sm::new(PacPio::INST, idx as u8))
    }
}

impl ServerImpl {
    fn new() -> Self {
        let peripherals = pac::Peripherals::take()
            .expect("PIO peripherals should be available");

        reset_hw!(peripherals.RESETS, [pio0, pio1]);

        Self {
            programs: Programs::default(),
            pio0: Pio::new(peripherals.PIO0),
            pio1: Pio::new(peripherals.PIO1),
        }
    }

    fn free_state_machine(&mut self) -> Option<Sm> {
        self.pio0
            .free_state_machine()
            .or_else(|| self.pio1.free_state_machine())
    }
}

impl idl::InOrderPioImpl for ServerImpl {
    fn create_sm(
        &mut self,
        _msg: &userlib::RecvMessage,
        program_ident: PioProgram,
    ) -> Result<Sm, RequestError<PioError>> {
        let sm = self
            .free_state_machine()
            .ok_or_else(|| RequestError::from(PioError::NoStateMachine))?;
        let program = &self.programs[program_ident];
        match sm.get_inst() {
            PioInst::Pio0 => self.pio0.install(sm.get_sm_idx(), program),
            PioInst::Pio1 => self.pio1.install(sm.get_sm_idx(), program),
        }?;
        Ok(sm)
    }
}

impl NotificationHandler for ServerImpl {
    fn current_notification_mask(&self) -> u32 {
        0
    }

    fn handle_notification(&mut self, _bits: userlib::NotificationBits) {
        unreachable!()
    }
}

#[unsafe(export_name = "main")]
fn main() -> ! {
    let mut incoming = [0; idl::INCOMING_SIZE];

    let mut server = ServerImpl::new();

    loop {
        idol_runtime::dispatch(&mut incoming, &mut server);
    }
}

mod idl {
    use drv_pio_api::{PioError, PioProgram, Sm};
    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}
