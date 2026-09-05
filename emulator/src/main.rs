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

//! A virtual IC-7100 on a pseudo-terminal.
//!
//! ```text
//! cargo run -p emulator                    # prints PTY_SLAVE=/dev/pts/N
//! cargo run -p emulator -- --address 94    # a second radio on the bus
//! ```

use std::io::{Read, Write};
use std::time::Instant;

use cat_framework::civ::CivFormat;
use cat_framework::CatFramework;
use emulator::{civ_loop, meter, pty::PtyPair, EmulatorError};
use radio::Ic7100Radio;

fn main() {
    if let Err(e) = run() {
        eprintln!("emulator: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), EmulatorError> {
    let mut address = 0x88u8;
    let mut seed = 7u64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            // Hex, because every CI-V address in every document is
            // written that way and asking for 136 would be unkind.
            "--address" => {
                if let Some(v) = args.next() {
                    address = u8::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or(0x88);
                }
            }
            "--seed" => {
                if let Some(v) = args.next() {
                    seed = v.parse().unwrap_or(7);
                }
            }
            _ => {}
        }
    }

    let mut pty = PtyPair::new()?;
    println!("PTY_SLAVE={}", pty.slave_path());
    println!("CIV_ADDRESS={address:02X}");
    let mut port = pty.take_master();

    // One band, so the meters and anything else rendering it describe the
    // same radio.
    let band = band_for(seed);
    let mut radio = CatFramework::with_format(
        Ic7100Radio::at_address(address),
        CivFormat::for_radio(address),
    );

    let started = Instant::now();
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 512];

    loop {
        match port.read(&mut chunk) {
            Ok(0) => continue,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
            // A PTY with nothing on the far end times out constantly.
            // That is the ordinary state, not a fault.
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(EmulatorError::Io(e)),
        }
        civ_loop::trim(&mut buffer);

        while let Some((frame, used)) = civ_loop::take_frame(&buffer) {
            buffer.drain(..used);

            // The meters, before answering: a console that reads the
            // S-meter must get what this radio is hearing *now*, and the
            // band moves with the dial.
            let t = started.elapsed().as_secs_f64();
            let state = radio.radio().state();
            let meters = if state.transmitting {
                meter::transmitting(state.levels.rf_power)
            } else {
                meter::receiving(&band, state.frequency(), t)
            };
            radio.radio_mut().set_meters(meters);

            let mut reply = Vec::new();
            match radio.process_frame(&frame, &mut reply) {
                Ok(_) if !reply.is_empty() => {
                    port.write_all(&reply)?;
                    port.flush()?;
                }
                // A frame this radio does not recognise. A real one on a
                // shared bus stays quiet rather than answering somebody
                // else's traffic.
                _ => {}
            }
        }
    }
}

/// The band this radio hears.
///
/// Populated across the HF portion of its coverage. The VHF and UHF
/// segments are left empty on purpose: a synthetic band that put signals
/// every few kilohertz across 144 MHz would be a picture of a contest that
/// is not happening, and an operator checking a console against it would
/// learn the wrong thing about what quiet looks like.
fn band_for(seed: u64) -> cat_signal::synthetic::Band {
    cat_signal::synthetic::Band::populated(1_800_000, 30_000_000, 120, -125.0, seed)
}
