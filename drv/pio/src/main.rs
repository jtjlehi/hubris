// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! PIO Driver
//!
//!
//! Lifecycle of an SM:
//!
//! ```text
//! ╭───────╮      ┌────────────────┐      ┌───────────┐
//! │ Start │─────>│ Create program │─────>│ Create SM │
//! ╰───────╯      └────────────────┘      └───────────┘
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

use idol_runtime::NotificationHandler;

struct ServerImpl {}

impl idl::InOrderPioImpl for ServerImpl {
    fn create_sm(
        &mut self,
        _msg: &userlib::RecvMessage,
    ) -> Result<(), idol_runtime::RequestError<core::convert::Infallible>> {
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

    let mut server = ServerImpl {};

    loop {
        idol_runtime::dispatch(&mut incoming, &mut server);
    }
}

mod idl {
    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}
