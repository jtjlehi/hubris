// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

use counters::Count;
use derive_idol_err::IdolError;
pub use strum::EnumCount;
use userlib::{FromPrimitive, sys_send};
use zerocopy::{Immutable, IntoBytes};

#[derive(
    Debug, FromPrimitive, Immutable, IntoBytes, Clone, Copy, strum::EnumCount,
)]
#[repr(u8)]
pub enum PioProgram {
    PioBlink,
}

#[derive(Debug, FromPrimitive, Copy, Clone, Immutable, IntoBytes)]
pub struct Sm(u8);

impl Sm {
    pub fn new(inst: PioInst, sm: u8) -> Self {
        Self(match inst {
            PioInst::Pio0 => sm,
            PioInst::Pio1 => sm + 4,
        })
    }
    /// Get the PIO instance this state machine belongs to
    pub fn get_inst(self) -> PioInst {
        match self.0 / 4 {
            0 => PioInst::Pio0,
            1 => PioInst::Pio1,
            _ => unreachable!(),
        }
    }
    /// Get the index of this state machine
    pub fn get_sm_idx(self) -> u8 {
        self.0 % 4
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PioInst {
    Pio0,
    Pio1,
}

#[derive(
    Copy, Clone, Debug, FromPrimitive, Eq, PartialEq, IdolError, Count,
)]
pub enum PioError {
    InvalidProgram = 1,
    NoStateMachine = 2,
    NoInstructionMemory = 3,
}

include!(concat!(env!("OUT_DIR"), "/client_stub.rs"));
