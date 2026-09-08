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

//! This radio, as the console protocol sees it.
//!
//! # Seven round trips, on purpose
//!
//! CI-V has no command that reads several things at once — no `IF;`
//! equivalent. So a console's state costs a read per field, and the seven
//! meters cost seven more. That is the protocol, not a shortcoming here,
//! and it is why this reads the meters as one group: a console must not
//! show an S-meter from one moment beside an SWR from another, and the
//! only way to get close on CI-V is to read them together and publish
//! them together.

use cat_native::{Command, MeterKind, MeterSample, RadioState};
use cat_rigctl::native_bridge::NativeRadio;
use cat_transport_core::CatSession;
use radio::{Ic7100, Mode};

/// This radio, as the console protocol sees it.
pub struct ConsoleIc7100<S: CatSession>(pub Ic7100<S>);

#[async_trait::async_trait(?Send)]
impl<S> NativeRadio for ConsoleIc7100<S>
where
    S: CatSession,
    S::Error: std::error::Error + 'static,
{
    async fn state(&mut self) -> Option<RadioState> {
        let hz = self.0.frequency().await.ok()?;
        let (mode, _filter) = self.0.mode().await.ok()?;
        let (split, _duplex) = self
            .0
            .split_and_duplex()
            .await
            .unwrap_or((false, radio::Duplex::Simplex));
        let transmitting = self.0.transmitting().await.unwrap_or(false);

        // All seven, together. A failed read drops the meters rather than
        // the whole state: a console can draw dashes for a meter and can
        // do nothing useful with a frequency it did not get.
        let meters = match self.0.meters().await {
            Ok(m) => vec![
                MeterSample {
                    kind: MeterKind::S,
                    raw: u16::from(m.s),
                },
                MeterSample {
                    kind: MeterKind::Po,
                    raw: u16::from(m.po),
                },
                MeterSample {
                    kind: MeterKind::Swr,
                    raw: u16::from(m.swr),
                },
                MeterSample {
                    kind: MeterKind::Alc,
                    raw: u16::from(m.alc),
                },
                MeterSample {
                    kind: MeterKind::Comp,
                    raw: u16::from(m.comp),
                },
                MeterSample {
                    kind: MeterKind::Vdd,
                    raw: u16::from(m.vd),
                },
                MeterSample {
                    kind: MeterKind::Id,
                    raw: u16::from(m.id),
                },
            ],
            Err(_) => Vec::new(),
        };

        Some(RadioState {
            vfo_a_hz: hz,
            // CI-V reads the *selected* VFO. Reporting it as B as well
            // would be inventing a reading, so B mirrors A until there is
            // a real read for it.
            vfo_b_hz: hz,
            mode: mode.to_shared().unwrap_or(cat_native::ModeId::Usb),
            split,
            transmitting,
            memory_channel: None,
            // The twin PBT is two controls, not one signed shift, and
            // reporting either half as "the IF shift" would put a number
            // in a cell that means something else.
            if_shift_hz: None,
            // Filters are selected as FIL1/2/3, not by width, so there is
            // no width to report.
            filter_width_hz: None,
            meters,
            // This radio's console does not read the occasional-settings
            // block. `None` says so; a console draws dashes rather than
            // its own struct defaults.
            levels: None,
        })
    }

    async fn apply(&mut self, command: &Command) -> Result<(), String> {
        match command {
            Command::SetFrequency { hz, .. } | Command::Retune { hz } => {
                self.0.set_frequency(*hz).await.map_err(|e| e.to_string())
            }
            Command::SetMode { mode } => match Mode::from_shared(*mode) {
                Some(m) => self.0.set_mode(m).await.map_err(|e| e.to_string()),
                None => Err("this radio has no such mode".to_string()),
            },
            Command::SetSplit { enabled } => {
                self.0.set_split(*enabled).await.map_err(|e| e.to_string())
            }
            Command::SetMemoryChannel { channel } => match u8::try_from(*channel) {
                Ok(c) => self.0.select_memory(c).await.map_err(|e| e.to_string()),
                Err(_) => Err("memory channel out of range".to_string()),
            },
            // Reads are answered from the published state, never sent.
            Command::ReadMeter { .. } | Command::ReadState | Command::ReadDevices => Ok(()),
            // Handled by `NativeShared` against its device directory and
            // never queued here. Kept explicit rather than swept into a
            // `_` arm, so the next command added to the protocol fails to
            // compile here instead of being silently ignored.
            Command::AttachDevice { .. } => Err("a device attach is not a CAT command".to_string()),
            Command::SetIfShift { .. } => {
                Err("this radio's twin PBT is two controls, not one shift".to_string())
            }
            Command::SetFilterWidth { .. } => {
                Err("this radio selects filters as FIL1/2/3, not by width".to_string())
            }
        }
    }
}
