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

use core::{cell::OnceCell, ops::Index};

use drv_pio_api::{EnumCount, PioError, PioProgram, Sm};
use idol_runtime::{NotificationHandler, RequestError};

type Program = pio::Program<{ pio::RP2040_MAX_PROGRAM_SIZE }>;

#[derive(Debug, Default)]
struct Programs([OnceCell<Program>; PioProgram::COUNT]);

impl Index<PioProgram> for Programs {
    type Output = Program;

    fn index(&self, index: PioProgram) -> &Self::Output {
        self.0[index as usize].get_or_init(|| match index {
            PioProgram::PioBlink => todo!(),
        })
    }
}

#[derive(Debug, Default)]
struct ServerImpl {
    programs: Programs,
}

impl ServerImpl {
    fn new() -> Self {
        todo!()
    }
}

impl idl::InOrderPioImpl for ServerImpl {
    fn create_sm(
        &mut self,
        _msg: &userlib::RecvMessage,
        program_ident: PioProgram,
    ) -> Result<Sm, RequestError<PioError>> {
        let _program = &self.programs[program_ident];
        todo!()
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
