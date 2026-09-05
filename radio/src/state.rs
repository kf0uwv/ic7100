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

//! What an IC-7100 is doing.
//!
//! The state a CI-V command reads or changes. Defaults are the radio's own
//! power-on condition where the manual states one, and a plausible resting
//! value where it does not — a state that started somewhere impossible
//! would make an emulator lie in its first frame.

use crate::mode::{Filter, Mode};

/// Which VFO is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vfo {
    #[default]
    A,
    B,
}

/// VFO or memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tuning {
    #[default]
    Vfo,
    Memory,
}

/// Duplex, for the repeater bands this radio reaches.
///
/// A TS-570D has no such thing. It matters here because this radio works
/// 2 m and 70 cm, where an operator lives on repeaters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Duplex {
    #[default]
    Simplex,
    /// DUP−: transmit below the receive frequency.
    Minus,
    /// DUP+: transmit above it.
    Plus,
}

/// Every level this radio reports over `14 xx`, in raw 0-255 counts.
///
/// Raw rather than converted, because the conversion differs per level and
/// several are not linear — the CW pitch runs 300 Hz to 900 Hz over the
/// same 0-255 the AF gain uses for a volume. Keeping the radio's own
/// number here means one place converts, at the point something needs
/// engineering units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Levels {
    pub af: u8,
    pub rf_gain: u8,
    pub squelch: u8,
    pub noise_reduction: u8,
    pub rf_power: u8,
    pub mic_gain: u8,
    /// 0 = 300 Hz, 128 = 600 Hz, 255 = 900 Hz.
    pub cw_pitch: u8,
    /// 0 = 6 WPM, 255 = 48 WPM.
    pub key_speed: u8,
    /// 0 lowest, 128 centre, 255 highest.
    pub notch: u8,
}

impl Default for Levels {
    fn default() -> Self {
        Self {
            // Mid volume, full RF gain, squelch open: what a radio is set
            // to when somebody has just switched it on and is listening.
            af: 96,
            rf_gain: 255,
            squelch: 0,
            noise_reduction: 0,
            rf_power: 255,
            mic_gain: 128,
            // 600 Hz, this radio's centre and the usual sidetone.
            cw_pitch: 128,
            // Around 20 WPM on the 6-48 scale.
            key_speed: 80,
            notch: 128,
        }
    }
}

/// Every meter this radio reports over `15 xx`, in raw counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Meters {
    /// 0 = S0, 120 = S9, 241 = S9+60 dB.
    pub s: u8,
    pub po: u8,
    pub swr: u8,
    pub alc: u8,
    pub comp: u8,
    /// 0 = 0 V, 13 = 10 V, 241 = 16 V.
    pub vd: u8,
    /// 0 = 0 A, 97 = 10 A, 146 = 15 A, 241 = 25 A.
    pub id: u8,
}

impl Meters {
    /// The meters of a radio that is receiving and idle.
    ///
    /// Transmit meters read zero on receive, which is not a missing value
    /// but the true one — an ALC reading while not transmitting would be
    /// the suspicious number.
    pub fn receiving() -> Self {
        Self {
            s: 0,
            po: 0,
            swr: 0,
            alc: 0,
            comp: 0,
            // 13.8 V, the supply this radio is specified at, on the
            // manual's 0 V = 0, 10 V = 13 scale.
            vd: 18,
            id: 0,
        }
    }
}

/// The whole of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Ic7100State {
    pub power_on: bool,
    pub vfo_a_hz: u64,
    pub vfo_b_hz: u64,
    pub selected: Vfo,
    pub tuning: Tuning,
    pub mode: Mode,
    pub filter: Filter,
    pub memory_channel: u8,
    pub split: bool,
    pub duplex: Duplex,
    pub transmitting: bool,
    /// The tuning step, as `10 xx`'s sub-command numbers it.
    pub tuning_step: u8,
    /// 0 = off, 12 = the 12 dB pad. The manual gives only those two.
    pub attenuator: u8,
    /// 0 off, 1 on (or preamp 1 on HF), 2 preamp 2.
    pub preamp: u8,
    /// 1 fast, 2 mid, 3 slow.
    pub agc: u8,
    pub noise_blanker: bool,
    pub noise_reduction_on: bool,
    pub auto_notch: bool,
    pub manual_notch: bool,
    pub speech_compressor: bool,
    pub vox: bool,
    pub break_in: bool,
    pub dial_lock: bool,
    pub levels: Levels,
    pub meters: Meters,
    /// This radio's own CI-V address, as `19 00` reports it.
    pub civ_address: u8,
}

impl Default for Ic7100State {
    fn default() -> Self {
        Self {
            power_on: true,
            // 14.100 MHz: in the 20 m band this radio's specification
            // lists, and a frequency an operator would recognise as a
            // starting point rather than as a placeholder.
            vfo_a_hz: 14_100_000,
            vfo_b_hz: 14_200_000,
            selected: Vfo::A,
            tuning: Tuning::Vfo,
            mode: Mode::Usb,
            filter: Filter::Fil1,
            memory_channel: 1,
            split: false,
            duplex: Duplex::Simplex,
            transmitting: false,
            // 10 Hz, sub-command 00.
            tuning_step: 0,
            attenuator: 0,
            preamp: 0,
            // Mid.
            agc: 2,
            noise_blanker: false,
            noise_reduction_on: false,
            auto_notch: false,
            manual_notch: false,
            speech_compressor: false,
            vox: false,
            break_in: false,
            dial_lock: false,
            levels: Levels::default(),
            meters: Meters::receiving(),
            civ_address: 0x88,
        }
    }
}

impl Ic7100State {
    /// The frequency of whichever VFO is selected.
    pub fn frequency(&self) -> u64 {
        match self.selected {
            Vfo::A => self.vfo_a_hz,
            Vfo::B => self.vfo_b_hz,
        }
    }

    /// Set the selected VFO's frequency.
    pub fn set_frequency(&mut self, hz: u64) {
        match self.selected {
            Vfo::A => self.vfo_a_hz = hz,
            Vfo::B => self.vfo_b_hz = hz,
        }
    }

    /// The frequency this radio would transmit on.
    ///
    /// Split puts transmit on the other VFO. Worth its own method because
    /// "which frequency" has two answers on this radio and a caller that
    /// picked the wrong one would key up somewhere unintended.
    pub fn transmit_frequency(&self) -> u64 {
        if !self.split {
            return self.frequency();
        }
        match self.selected {
            Vfo::A => self.vfo_b_hz,
            Vfo::B => self.vfo_a_hz,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_radio_is_somewhere_real() {
        // A state that started somewhere impossible would make an emulator
        // lie in its first frame, and every console tested against it
        // would agree.
        let s = Ic7100State::default();
        assert!(crate::capabilities::covers(s.vfo_a_hz));
        assert!(crate::capabilities::covers(s.vfo_b_hz));
        assert!(s.power_on);
        assert!(!s.transmitting);
    }

    #[test]
    fn the_selected_vfo_is_the_one_read_and_written() {
        let mut s = Ic7100State::default();
        s.set_frequency(7_100_000);
        assert_eq!(s.vfo_a_hz, 7_100_000);
        assert_eq!(s.frequency(), 7_100_000);

        s.selected = Vfo::B;
        assert_eq!(s.frequency(), s.vfo_b_hz);
        s.set_frequency(21_200_000);
        assert_eq!(s.vfo_b_hz, 21_200_000);
        // A's frequency is untouched: selecting B must not move A.
        assert_eq!(s.vfo_a_hz, 7_100_000);
    }

    #[test]
    fn split_transmits_on_the_other_vfo() {
        // The question with two answers. A caller that took the receive
        // frequency would key up on the wrong one.
        let mut s = Ic7100State::default();
        assert_eq!(s.transmit_frequency(), s.vfo_a_hz);

        s.split = true;
        assert_eq!(s.transmit_frequency(), s.vfo_b_hz);

        s.selected = Vfo::B;
        assert_eq!(s.transmit_frequency(), s.vfo_a_hz);
    }

    #[test]
    fn a_receiving_radio_reads_zero_on_its_transmit_meters() {
        // Not missing values: the true ones. An ALC reading while not
        // transmitting would be the suspicious number.
        let m = Meters::receiving();
        assert_eq!((m.po, m.swr, m.alc, m.comp, m.id), (0, 0, 0, 0, 0));
        // The supply is there whether or not it is transmitting.
        assert!(m.vd > 0);
    }

    #[test]
    fn the_default_levels_are_a_radio_somebody_is_listening_to() {
        let l = Levels::default();
        assert!(l.af > 0, "silent");
        assert_eq!(l.rf_gain, 255, "RF gain backed off on a fresh radio");
        assert_eq!(l.squelch, 0, "squelch closed on a fresh radio");
        // 600 Hz, the centre of this radio's 300-900 Hz range.
        assert_eq!(l.cw_pitch, 128);
    }
}
