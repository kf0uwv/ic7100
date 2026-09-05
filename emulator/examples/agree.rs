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

//! Does this emulator's S-meter agree with the band it is receiving?
//!
//! ```text
//! cargo run -p emulator --example agree
//! ```
//!
//! Prints the strongest signals and what the meter reads tuned onto each,
//! over several moments — a mode with an envelope is *supposed* to come
//! and go, and a reading that never moved would be the suspicious one.

use cat_framework::capabilities::SUnitScale;
use radio::capabilities::IC7100_S_UNITS;

fn main() {
    let band = cat_signal::synthetic::Band::populated(1_800_000, 30_000_000, 120, -125.0, 7);
    let scale: SUnitScale = IC7100_S_UNITS;

    let mut emitters: Vec<_> = band.emitters().iter().collect();
    emitters.sort_by(|a, b| b.level_dbm.partial_cmp(&a.level_dbm).unwrap());

    println!(
        "{:>13}  {:>7}  {:>10}  {:>6}  S-meter over 8 s",
        "frequency", "level", "mode", "S-unit"
    );
    for e in emitters.iter().take(10) {
        let readings: Vec<String> = (0..8)
            .map(|s| {
                emulator::meter::receiving(&band, e.frequency_hz, f64::from(s))
                    .s
                    .to_string()
            })
            .collect();
        let now = emulator::meter::receiving(&band, e.frequency_hz, 0.0).s;
        println!(
            "{:>10.4} MHz  {:>3.0} dBm  {:>10}  {:>6}  {}",
            e.frequency_hz as f64 / 1e6,
            e.level_dbm,
            format!("{:?}", e.emission),
            scale.label(u16::from(now)),
            readings.join(" ")
        );
    }
}
