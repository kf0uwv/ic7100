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

//! What an IC-7100 is.
//!
//! Every number here comes from the IC-7100 Full Manual, and the comment
//! beside it says where. A declaration that agreed with the radio by
//! coincidence would be worse than none, because everything downstream
//! trusts it: which tabs a console grows, which controls it offers, how it
//! scales a meter.

use cat_framework::capabilities::*;

pub use cat_framework::capabilities::{EndpointRole, ModeId, SignalSupport};
pub use cat_framework::installation::{AudioOrigin, Installation, InstalledSource, SourceState};
pub use cat_signal::SignalCapability;

const MODES: &[ModeDescriptor] = &[
    ModeDescriptor {
        id: ModeId::Lsb,
        label: "LSB",
        kind: ModeKind::Ssb,
        sideband: Some(Sideband::Lower),
        default_bandwidth_hz: 2400,
    },
    ModeDescriptor {
        id: ModeId::Usb,
        label: "USB",
        kind: ModeKind::Ssb,
        sideband: Some(Sideband::Upper),
        default_bandwidth_hz: 2400,
    },
    ModeDescriptor {
        id: ModeId::Am,
        label: "AM",
        kind: ModeKind::Am,
        sideband: None,
        default_bandwidth_hz: 6000,
    },
    ModeDescriptor {
        id: ModeId::CwUpper,
        label: "CW",
        kind: ModeKind::Cw,
        sideband: Some(Sideband::Upper),
        default_bandwidth_hz: 500,
    },
    ModeDescriptor {
        id: ModeId::CwLower,
        label: "CW-R",
        kind: ModeKind::Cw,
        sideband: Some(Sideband::Lower),
        default_bandwidth_hz: 500,
    },
    ModeDescriptor {
        id: ModeId::RttyLsb,
        label: "RTTY",
        kind: ModeKind::Data,
        sideband: Some(Sideband::Lower),
        default_bandwidth_hz: 500,
    },
    ModeDescriptor {
        id: ModeId::RttyUsb,
        label: "RTTY-R",
        kind: ModeKind::Data,
        sideband: Some(Sideband::Upper),
        default_bandwidth_hz: 500,
    },
    ModeDescriptor {
        id: ModeId::Fm,
        label: "FM",
        kind: ModeKind::Fm,
        sideband: None,
        default_bandwidth_hz: 12000,
    },
    // D-STAR. `C4fm` is the shared vocabulary's one digital-voice entry --
    // Yaesu's word for a Yaesu mode -- and this radio's operators mean
    // "the digital mode this radio has" by it. See `mode.rs`.
    ModeDescriptor {
        id: ModeId::C4fm,
        label: "DV",
        kind: ModeKind::DigitalVoice,
        sideband: None,
        default_bandwidth_hz: 6250,
    },
];

/// Seven meters, each with the manual's own calibration.
///
/// More than either other radio in this fleet, and the reason a console
/// derived from this document looks different on an IC-7100 without
/// anybody writing an IC-7100 console.
///
/// Every `raw_range` is 0-255 because CI-V reports all of them as two BCD
/// bytes over that range (20-5, command 15). What differs is what the
/// number *means*, and that lives in the S-unit table below and in the
/// notes here.
const METERS: &[MeterDescriptor] = &[
    // 15 02: 0000=S0, 0120=S9, 0241=S9+60 dB.
    MeterDescriptor {
        kind: MeterKind::S,
        raw_range: RawRange::new(0, 255),
        active_on_transmit: false,
        s_units: Some(IC7100_S_UNITS),
    },
    // 15 11: 0000=0%, 0143=50%, 0213=100%.
    MeterDescriptor {
        kind: MeterKind::Po,
        raw_range: RawRange::new(0, 255),
        active_on_transmit: true,
        s_units: None,
    },
    // 15 12: 0000=SWR1.0, 0048=1.5, 0080=2.0, 0120=3.0.
    MeterDescriptor {
        kind: MeterKind::Swr,
        raw_range: RawRange::new(0, 255),
        active_on_transmit: true,
        s_units: None,
    },
    // 15 13: 0000=Min. to 0120=Max.
    MeterDescriptor {
        kind: MeterKind::Alc,
        raw_range: RawRange::new(0, 255),
        active_on_transmit: true,
        s_units: None,
    },
    // 15 14: 0000=0 dB, 0130=15 dB, 0241=30 dB.
    MeterDescriptor {
        kind: MeterKind::Comp,
        raw_range: RawRange::new(0, 255),
        active_on_transmit: true,
        s_units: None,
    },
    // 15 15: 0000=0 V, 0013=10 V, 0241=16 V. Supply voltage -- a meter
    // neither other radio in this fleet reports at all.
    MeterDescriptor {
        kind: MeterKind::Vdd,
        raw_range: RawRange::new(0, 255),
        active_on_transmit: true,
        s_units: None,
    },
    // 15 16: 0000=0, 0097=10, 0146=15, 0241=25 (amps).
    MeterDescriptor {
        kind: MeterKind::Id,
        raw_range: RawRange::new(0, 255),
        active_on_transmit: true,
        s_units: None,
    },
];

/// Where each S-unit falls on this radio's raw scale.
///
/// From 20-5: `0000=S0, 0120=S9, 0241=S9+60 dB`. Only three points are
/// given, so the units between them are interpolated linearly — 120 raw
/// counts across nine S-units is 13.33 each, and above S9 it is 121 counts
/// across 60 dB.
///
/// **This is not the TS-570D's shape.** That radio's table is irregular
/// and had to be transcribed count by count; this one the manual describes
/// as a straight line between calibration points, so the thresholds below
/// are that line evaluated at each boundary and rounded.
pub const IC7100_S_UNITS: SUnitScale = SUnitScale::new([
    // Derived, and the derivation matters because the first attempt got
    // it wrong in a way a spot check would miss.
    //
    // The manual gives three points: raw 0 is S0, raw 120 is S9, raw 241
    // is S9+60 dB. So below S9 an S-unit is 120/9 = 13.33 counts, and
    // above it 10 dB is (241-120)/60*10 = 20.17 counts. That puts the
    // labels at their *nominal* raw values:
    //
    //   S0..S9      0.0  13.3  26.7  40.0  53.3  66.7  80.0  93.3 106.7 120.0
    //   S9+10..+60                140.2 160.3 180.5 200.7 220.8 241.0
    //
    // These thresholds are the **inclusive top** of each label, so each
    // one is the midpoint between its nominal value and the next. Using
    // the unit boundaries instead -- which is what the first version did
    // -- puts raw 120 at the top of S8, so the radio's own S9 calibration
    // point labels as S8.
    7,
    20,
    33,
    47,
    60,
    73,
    87,
    100,
    113,
    // S9, then the +10 and +20 dB divisions.
    130,
    150,
    170,
    // The same line continued to the manual's third calibration point.
    // Nominal S9+40/50/60 are 200.7, 220.8 and 241.0, so these are the
    // midpoints again -- and the last is `u16::MAX` rather than 241,
    // because a reading past the top of the meter should peg there
    // rather than fall through to S0.
    //
    // The labels used to stop at S9+30, so everything from raw 171 up --
    // the top third of this radio's range -- drew as S9+30, up to 30 dB
    // weaker than the radio was reporting.
    191,
    211,
    231,
    u16::MAX,
]);

const ENDPOINTS: &[EndpointDescriptor] = &[
    // The rear [REMOTE] jack: CI-V on a 3.5 mm plug, and the reason a
    // CT-17 level converter exists.
    EndpointDescriptor {
        role: EndpointRole::Cat,
        required: true,
        shareable_with: &[],
    },
];

/// The Icom IC-7100.
pub const IC7100: RadioCapabilities = RadioCapabilities {
    model: "Icom IC-7100",
    endpoints: EndpointSet::new(ENDPOINTS),
    vfos: VfoCapability {
        count: 2,
        split: true,
        // RIT on this radio is the [Δ]/RIT control, ±9.999 kHz.
        rit_hz: Some(9_999),
        xit_hz: Some(9_999),
    },
    modes: MODES,
    // 20-3, command 10: the radio's own tuning-step list.
    tuning_steps_hz: &[
        10, 100, 1_000, 5_000, 6_250, 9_000, 10_000, 12_500, 20_000, 25_000, 50_000, 100_000,
        1_000_000,
    ],
    // Specification page: receive 0.030000-199.999999 MHz *and*
    // 400.000000-470.000000 MHz.
    //
    // The capability model carries one contiguous range, so this is the
    // outer envelope. It over-claims the 200-400 MHz gap, and that is
    // recorded rather than hidden -- see `covers` below, which is the
    // honest answer and what a band bar should ask.
    rx_range: FrequencyRange {
        min_hz: 30_000,
        max_hz: 470_000_000,
    },
    filters: FILTERS,
    meters: MeterSet::new(METERS),
    memory: Some(MemoryCapability {
        // 20-3, command 08: 0001 to 0109, where 0001-0099 are M-CH01 to
        // M-CH99 and 0100 upward are the scan edges and call channels.
        // Consoles are offered the regular channels; the rest are not
        // memories an operator picks by number.
        channels: RawRange::new(1, 99),
        named: true,
        stores_mode: true,
        scan: true,
    }),
    // The set mode reached over CI-V as 1A 05 nnnn. The manual's list runs
    // past 0100, and this is the count of items a console may walk.
    menu: Some(MenuCapability {
        item_count: 112,
        writable: true,
    }),
    // No IF tap and no spectrum over CI-V. The IC-7100 has no band scope
    // at all -- unlike its stablemates the IC-7300 and IC-7610, which do
    // and expose it. A negative fact, stated, because it is why this
    // radio's console has no waterfall.
    signal: SignalSupport::None,
};

const FILTERS: FilterCapability = FilterCapability {
    // 20-4, command 14 07/08: the twin PBT pair, each 0000-0255 with 0128
    // centre. Reported as the shift one control provides.
    if_shift_hz: Some(1_200),
    // Three IF filters per mode (FIL1/2/3), selected by the filter byte
    // rather than by width, so there is no list of widths to publish.
    widths_hz: None,
    // 16 48 manual notch, and 14 0D sets its position.
    notch: true,
};

/// Whether this radio actually receives at `hz`.
///
/// `rx_range` is the outer envelope and includes the 200-400 MHz gap the
/// specification page leaves out. This is the real answer, and a band bar
/// or a tune command should ask it rather than the envelope.
pub fn covers(hz: u64) -> bool {
    (30_000..=199_999_999).contains(&hz) || (400_000_000..=470_000_000).contains(&hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_meters_are_the_seven_the_manual_lists() {
        // 15 02, 11, 12, 13, 14, 15, 16. More than either other radio in
        // this fleet, which is the point: a console derived from this
        // document shows an IC-7100's instruments without anybody writing
        // an IC-7100 console.
        let kinds: Vec<MeterKind> = IC7100.meters.meters.iter().map(|m| m.kind).collect();
        assert_eq!(
            kinds,
            vec![
                MeterKind::S,
                MeterKind::Po,
                MeterKind::Swr,
                MeterKind::Alc,
                MeterKind::Comp,
                MeterKind::Vdd,
                MeterKind::Id,
            ]
        );
    }

    #[test]
    fn the_s_scale_puts_s9_where_the_manual_does() {
        // 20-5: 0000=S0, 0120=S9, 0241=S9+60 dB. A console labelling raw
        // 120 anything but S9 would disagree with the radio's own display.
        // The manual's three calibration points, which is what the scale
        // is built from and therefore what it must reproduce exactly.
        assert_eq!(IC7100_S_UNITS.label(0), "S0");
        assert_eq!(IC7100_S_UNITS.label(120), "S9");

        // The derived nominals for the divisions above S9.
        assert_eq!(IC7100_S_UNITS.label(140), "S9+10");
        assert_eq!(IC7100_S_UNITS.label(160), "S9+20");
        assert_eq!(IC7100_S_UNITS.label(181), "S9+30");
        assert_eq!(IC7100_S_UNITS.label(200), "S9+40");
        assert_eq!(IC7100_S_UNITS.label(220), "S9+50");

        // The manual's third point, raw 241 = S9+60 dB. This used to read
        // S9+30 because the shared label set stopped there and the top
        // label absorbed everything above it -- a signal 60 dB over S9
        // was reported as half that. `S_UNIT_LABELS` now runs to S9+60,
        // so the scale reproduces all three of the manual's points.
        assert_eq!(IC7100_S_UNITS.label(241), "S9+60");
    }

    #[test]
    fn the_s_scale_is_monotonic_across_its_whole_range() {
        // A threshold out of order would make a stronger signal read as a
        // weaker one somewhere in the middle, which is the kind of fault
        // that survives a spot check.
        let mut last = 0usize;
        for raw in 0..=255u16 {
            let i = S_UNIT_LABELS
                .iter()
                .position(|l| *l == IC7100_S_UNITS.label(raw))
                .unwrap();
            assert!(i >= last, "raw {raw} went backwards");
            last = i;
        }
    }

    #[test]
    fn this_radio_reaches_where_the_specification_says_and_not_between() {
        // HF through UHF with a real hole in it. A band bar that trusted
        // the envelope would offer 300 MHz, which this radio does not
        // receive.
        for hz in [30_000, 14_074_000, 145_000_000, 199_999_999, 435_000_000] {
            assert!(covers(hz), "{hz} should be covered");
        }
        for hz in [29_999, 200_000_000, 300_000_000, 399_999_999, 470_000_001] {
            assert!(!covers(hz), "{hz} should not be covered");
        }
    }

    #[test]
    fn the_envelope_contains_everything_covers_accepts() {
        // The envelope may over-claim; it must never under-claim, or a
        // frequency this radio reaches would be refused before it was
        // asked for.
        for hz in [30_000, 145_000_000, 435_000_000, 470_000_000] {
            assert!(IC7100.rx_range.contains(hz), "{hz}");
        }
    }

    #[test]
    fn it_declares_no_spectrum_because_it_has_none() {
        // The IC-7100 has no band scope, unlike the IC-7300 and IC-7610.
        // Saying so is what stops a console growing a waterfall panel that
        // could never fill.
        assert_eq!(IC7100.signal, SignalSupport::None);
    }

    #[test]
    fn every_declared_mode_maps_to_one_this_radio_has() {
        // A capability set that offered a console a mode the command table
        // cannot send is a console with a control that fails when used.
        for descriptor in IC7100.modes {
            assert!(
                crate::mode::Mode::from_shared(descriptor.id).is_some(),
                "{} is offered but cannot be applied",
                descriptor.label
            );
        }
    }
}
