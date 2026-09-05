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

//! This radio's modes and its IF filters.
//!
//! From the IC-7100 Full Manual, 20-11 ("Operating mode", commands 01, 04
//! and 06) and the command table at 20-3.

use cat_framework::capabilities::ModeId;

/// A mode, as CI-V numbers them.
///
/// The values are the manual's, and the gap between `RttyReverse` (0x08)
/// and `Dv` (0x17) is real: Icom left room for modes this radio does not
/// have. Writing them out rather than deriving from a count is what keeps
/// that gap honest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Mode {
    Lsb = 0x00,
    Usb = 0x01,
    Am = 0x02,
    Cw = 0x03,
    Rtty = 0x04,
    Fm = 0x05,
    /// Wide FM. **Receive only** — the specification page lists WFM under
    /// "WFM: RX only", so a console must not offer transmit in it.
    Wfm = 0x06,
    CwReverse = 0x07,
    RttyReverse = 0x08,
    /// D-STAR digital voice. The mode neither other radio in this fleet
    /// has, and the reason this one gets a `DigitalVoice` kind at all.
    Dv = 0x17,
}

/// Which IF filter a mode is using.
///
/// Three per mode, and the manual notes the filter byte may be omitted on
/// commands 01 and 06 — in which case the radio picks FIL1 or the mode's
/// default respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Filter {
    Fil1 = 0x01,
    Fil2 = 0x02,
    Fil3 = 0x03,
}

impl Mode {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// The mode for a CI-V byte, or `None` for one this radio lacks.
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0x00 => Mode::Lsb,
            0x01 => Mode::Usb,
            0x02 => Mode::Am,
            0x03 => Mode::Cw,
            0x04 => Mode::Rtty,
            0x05 => Mode::Fm,
            0x06 => Mode::Wfm,
            0x07 => Mode::CwReverse,
            0x08 => Mode::RttyReverse,
            0x17 => Mode::Dv,
            _ => return None,
        })
    }

    /// The label an operator reads, in the form this radio's own display
    /// uses.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Lsb => "LSB",
            Mode::Usb => "USB",
            Mode::Am => "AM",
            Mode::Cw => "CW",
            Mode::Rtty => "RTTY",
            Mode::Fm => "FM",
            Mode::Wfm => "WFM",
            Mode::CwReverse => "CW-R",
            Mode::RttyReverse => "RTTY-R",
            Mode::Dv => "DV",
        }
    }

    /// Whether this radio will transmit in this mode.
    ///
    /// WFM is receive-only. A console that offered transmit in it would be
    /// offering a control the radio refuses, and finding that out by
    /// pressing it is a poor way to learn.
    pub fn can_transmit(self) -> bool {
        self != Mode::Wfm
    }

    /// The shared vocabulary's name for this mode.
    pub fn to_shared(self) -> Option<ModeId> {
        Some(match self {
            Mode::Lsb => ModeId::Lsb,
            Mode::Usb => ModeId::Usb,
            Mode::Am => ModeId::Am,
            Mode::Cw => ModeId::CwUpper,
            Mode::CwReverse => ModeId::CwLower,
            Mode::Rtty => ModeId::RttyLsb,
            Mode::RttyReverse => ModeId::RttyUsb,
            Mode::Fm => ModeId::Fm,
            Mode::Dv => ModeId::C4fm,
            // No shared name. WFM is a broadcast-listening mode and the
            // vocabulary has none; reporting it as FM would tell a console
            // it could transmit.
            Mode::Wfm => return None,
        })
    }

    /// This radio's mode for a shared name, or `None` if it has none.
    pub fn from_shared(id: ModeId) -> Option<Self> {
        Some(match id {
            ModeId::Lsb => Mode::Lsb,
            ModeId::Usb => Mode::Usb,
            ModeId::Am => Mode::Am,
            ModeId::CwUpper => Mode::Cw,
            ModeId::CwLower => Mode::CwReverse,
            ModeId::RttyLsb => Mode::Rtty,
            ModeId::RttyUsb => Mode::RttyReverse,
            ModeId::Fm => Mode::Fm,
            // `C4fm` is the vocabulary's digital-voice name. It is Yaesu's
            // word and this is D-STAR, but the shared type has one digital
            // voice entry and both radios' operators mean "the digital
            // mode this radio has" by it.
            ModeId::C4fm => Mode::Dv,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Mode; 10] = [
        Mode::Lsb,
        Mode::Usb,
        Mode::Am,
        Mode::Cw,
        Mode::Rtty,
        Mode::Fm,
        Mode::Wfm,
        Mode::CwReverse,
        Mode::RttyReverse,
        Mode::Dv,
    ];

    #[test]
    fn every_mode_byte_is_the_manuals() {
        // 20-11: 00 LSB, 01 USB, 02 AM, 03 CW, 04 RTTY, 05 FM, 06 WFM,
        // 07 CW-R, 08 RTTY-R, 17 DV. Written out against the source
        // because a mode byte that is off by one selects a real mode and
        // nothing anywhere reports an error.
        for (mode, byte) in [
            (Mode::Lsb, 0x00),
            (Mode::Usb, 0x01),
            (Mode::Am, 0x02),
            (Mode::Cw, 0x03),
            (Mode::Rtty, 0x04),
            (Mode::Fm, 0x05),
            (Mode::Wfm, 0x06),
            (Mode::CwReverse, 0x07),
            (Mode::RttyReverse, 0x08),
            (Mode::Dv, 0x17),
        ] {
            assert_eq!(mode.as_u8(), byte, "{mode:?}");
            assert_eq!(Mode::from_u8(byte), Some(mode));
        }
    }

    #[test]
    fn a_byte_this_radio_has_no_mode_for_is_refused() {
        // The gap between 08 and 17 is real. Guessing a mode from it would
        // put the radio somewhere nobody asked for.
        for byte in [0x09, 0x10, 0x16, 0x18, 0xFF] {
            assert_eq!(Mode::from_u8(byte), None, "{byte:#04X}");
        }
    }

    #[test]
    fn wfm_is_receive_only() {
        // The specification page says so. Every other mode transmits.
        assert!(!Mode::Wfm.can_transmit());
        for mode in ALL.into_iter().filter(|m| *m != Mode::Wfm) {
            assert!(mode.can_transmit(), "{mode:?}");
        }
    }

    #[test]
    fn the_shared_vocabulary_round_trips_every_mode_it_can_name() {
        for mode in ALL {
            match mode.to_shared() {
                Some(id) => assert_eq!(Mode::from_shared(id), Some(mode), "{mode:?}"),
                // WFM has no shared name, and that is the honest answer
                // rather than reporting it as FM.
                None => assert_eq!(mode, Mode::Wfm),
            }
        }
    }

    #[test]
    fn the_two_reversed_pairs_stay_distinct() {
        // CW/CW-R and RTTY/RTTY-R differ only by sideband, which is
        // exactly the pair a mapping collapses by accident.
        assert_ne!(Mode::Cw.to_shared(), Mode::CwReverse.to_shared());
        assert_ne!(Mode::Rtty.to_shared(), Mode::RttyReverse.to_shared());
    }

    #[test]
    fn every_mode_has_a_distinct_label() {
        let mut seen = std::collections::HashSet::new();
        for mode in ALL {
            assert!(seen.insert(mode.label()), "{:?} duplicates", mode.label());
        }
    }
}
