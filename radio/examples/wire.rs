// Copyright 2026 Matt Franklin
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Print what this radio actually says on the wire.
//!
//! ```text
//! cargo run -p radio --example wire
//! ```
//!
//! For reading against the manual's frame diagram at 20-2. Assertions
//! check that bytes are what a test expected; this is for checking that
//! what a test expected is what Icom documented, which is a different
//! question and the one that matters when a table is transcribed by hand.

use cat_framework::civ::CivFormat;
use cat_framework::wire_format::CatWireFormat;
use cat_framework::CatFramework;
use radio::{Ic7100Radio, Mode};

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let civ = CivFormat::default();
    let mut fw = CatFramework::with_format(Ic7100Radio::new(), civ);

    println!("{:<34}  {:<26}  reply", "what", "controller -> IC-7100");
    println!("{}", "-".repeat(96));

    let mut show = |what: &str, code: (u8, Option<u8>), data: &[u8]| {
        let request = civ.encode_request(code, data);
        let mut reply = Vec::new();
        let note = match fw.process_frame(&request, &mut reply) {
            Ok(_) => hex(&reply),
            Err(e) => format!("({e:?})"),
        };
        println!("{what:<34}  {:<26}  {note}", hex(&request));
    };

    show("read frequency", (0x03, None), &[]);
    show(
        "set 14.074 MHz",
        (0x05, None),
        &cat_framework::civ::encode_bcd(14_074_000, 5),
    );
    show("read mode", (0x04, None), &[]);
    show("set CW, FIL2", (0x06, None), &[Mode::Cw.as_u8(), 0x02]);
    show("read S-meter", (0x15, Some(0x02)), &[]);
    show("read Id meter", (0x15, Some(0x16)), &[]);
    show("read AF level", (0x14, Some(0x01)), &[]);
    show("split on", (0x0F, None), &[0x01]);
    show("read split/duplex", (0x0F, None), &[]);
    show("read this radio's address", (0x19, Some(0x00)), &[]);
    show(
        "300 MHz — in the coverage gap",
        (0x05, None),
        &cat_framework::civ::encode_bcd(300_000_000, 5),
    );
}
