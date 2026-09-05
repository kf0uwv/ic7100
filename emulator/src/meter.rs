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

//! Meters that agree with the band this radio is receiving.
//!
//! The same lesson as `ts570d`'s: an emulator answering a constant lets a
//! console pass every test and fail on hardware. Tune onto a signal and
//! the needle comes up, because it is the same signal.
//!
//! # This radio's scale, not another's
//!
//! The IC-7100 reports its S-meter over 0-255 with `0000=S0, 0120=S9,
//! 0241=S9+60 dB` (manual 20-5). A TS-570D reports 0-30 against an
//! irregular table. Mapping a signal through the wrong one gives a
//! plausible number that is several S-units out, so this inverts *this*
//! radio's calibration.

use cat_signal::synthetic::Band;
use radio::state::Meters;

/// The receive passband the meter integrates over.
///
/// An S-meter reads what got through the filter. 3 kHz is this radio's
/// declared SSB bandwidth, and using it is what makes tuning *onto* a
/// signal the thing that raises the needle rather than tuning near it.
pub const PASSBAND_HZ: u32 = 3_000;

/// S9, by convention on HF.
const S9_DBM: f32 = -73.0;

/// Decibels per S-unit below S9.
const DB_PER_S_UNIT: f32 = 6.0;

/// This radio's raw reading at S9. Manual 20-5.
const RAW_S9: f32 = 120.0;

/// Raw counts per S-unit below S9: 120 counts across nine units.
const RAW_PER_S_UNIT: f32 = RAW_S9 / 9.0;

/// Raw counts per dB above S9: (241 − 120) over 60 dB.
const RAW_PER_DB_OVER: f32 = (241.0 - RAW_S9) / 60.0;

/// The top of the scale. CI-V carries a meter as two BCD bytes and the
/// capability set says 0-255.
const RAW_MAX: f32 = 255.0;

/// The strongest thing in the passband at `dial_hz`, in dBm.
pub fn passband_peak_dbm(band: &Band, dial_hz: u64, t: f64) -> f32 {
    band.render(dial_hz, PASSBAND_HZ, 9, t)
        .into_iter()
        .fold(f32::NEG_INFINITY, f32::max)
}

/// This radio's raw S-meter reading for a signal of `dbm`.
pub fn raw_for_dbm(dbm: f32) -> u8 {
    if dbm.is_nan() {
        return 0;
    }
    if dbm == f32::INFINITY {
        return RAW_MAX as u8;
    }
    if dbm == f32::NEG_INFINITY {
        return 0;
    }
    let raw = if dbm <= S9_DBM {
        RAW_S9 - (S9_DBM - dbm) / DB_PER_S_UNIT * RAW_PER_S_UNIT
    } else {
        RAW_S9 + (dbm - S9_DBM) * RAW_PER_DB_OVER
    };
    raw.clamp(0.0, RAW_MAX) as u8
}

/// Every meter, for a radio receiving `band` at `dial_hz`.
///
/// The transmit meters read zero, which is the true value on receive and
/// not a missing one — an ALC reading while not transmitting would be the
/// suspicious number.
pub fn receiving(band: &Band, dial_hz: u64, t: f64) -> Meters {
    Meters {
        s: raw_for_dbm(passband_peak_dbm(band, dial_hz, t)),
        po: 0,
        swr: 0,
        alc: 0,
        comp: 0,
        // 13.8 V, this radio's specified supply, on the manual's
        // 0 V = 0, 10 V = 13 scale.
        vd: 18,
        id: 0,
    }
}

/// Every meter, for a radio transmitting at `power` (0-255).
///
/// A transmitting radio's meters are not a signal report: PO follows the
/// power setting, SWR sits where the antenna puts it, and Id follows PO
/// because current is what makes the power. Related rather than
/// independent, because on a real radio they are.
pub fn transmitting(power: u8) -> Meters {
    let fraction = f32::from(power) / 255.0;
    Meters {
        // No received signal while transmitting.
        s: 0,
        // 0213 is 100% on this radio's PO scale.
        po: (fraction * 213.0) as u8,
        // A well-matched antenna: 0048 is SWR 1.5, so a little under that.
        swr: 30,
        // ALC acting, as it does on voice peaks.
        alc: 60,
        comp: 0,
        // The supply sags a little under load, as a real one does.
        vd: 17,
        // 0097 is 10 A. Full power on this radio draws about 20 A, which
        // is roughly 146 on the manual's scale.
        id: (fraction * 146.0) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cat_framework::capabilities::SUnitScale;
    use cat_signal::synthetic::{Emission, Emitter};
    use radio::capabilities::IC7100_S_UNITS;

    fn band_with_carrier_at(hz: u64, dbm: f32) -> Band {
        Band::empty(-130.0, 1).with(Emitter::new(hz, Emission::Cw, dbm))
    }

    #[test]
    fn the_mapping_agrees_with_this_radios_own_scale() {
        // Not another radio's. A console labelling raw 120 "S9" and an
        // emulator that thought S9 was raw 20 would disagree about the
        // same signal, and every test between them would still pass.
        let scale: SUnitScale = IC7100_S_UNITS;
        for (dbm, want) in [
            (-73.0, "S9"),
            (-79.0, "S8"),
            (-85.0, "S7"),
            (-63.0, "S9+10"),
        ] {
            assert_eq!(scale.label(u16::from(raw_for_dbm(dbm))), want, "{dbm} dBm");
        }
    }

    #[test]
    fn tuning_onto_a_signal_raises_the_needle() {
        let band = band_with_carrier_at(14_100_000, -60.0);
        let on = receiving(&band, 14_100_000, 0.0).s;
        let off = receiving(&band, 14_200_000, 0.0).s;
        assert!(on > off, "on {on} vs off {off}");
        assert!(
            on >= 120,
            "a -60 dBm carrier should read at least S9, got {on}"
        );
    }

    #[test]
    fn a_quiet_band_rests_near_the_bottom() {
        let quiet = Band::empty(-135.0, 1);
        let raw = receiving(&quiet, 14_100_000, 0.0).s;
        assert!(raw <= 25, "a dead band read {raw}");
    }

    #[test]
    fn it_never_reads_past_the_top_of_the_scale() {
        // CI-V carries a meter as two BCD bytes and the capability set
        // says 0-255. A reading above that is a protocol violation as
        // well as a lie.
        // `s` is a `u8`, so "not above 255" is the type's guarantee and
        // asserting it proves nothing -- which is what the first version
        // of this test did. What is worth checking is that an enormous
        // signal *pegs* rather than wrapping round to a small number,
        // which an unclamped cast would do.
        let enormous = band_with_carrier_at(14_100_000, 40.0);
        assert_eq!(receiving(&enormous, 14_100_000, 0.0).s, 255);
        assert_eq!(raw_for_dbm(f32::INFINITY), 255);
        assert_eq!(raw_for_dbm(1000.0), 255);
    }

    #[test]
    fn the_reading_is_monotonic_in_signal_strength() {
        let mut last = 0;
        let mut dbm = -140.0;
        while dbm <= 0.0 {
            let raw = raw_for_dbm(dbm);
            assert!(raw >= last, "{dbm} dBm read {raw} after {last}");
            last = raw;
            dbm += 0.5;
        }
    }

    #[test]
    fn a_receiving_radio_shows_nothing_on_its_transmit_meters() {
        let band = band_with_carrier_at(14_100_000, -60.0);
        let m = receiving(&band, 14_100_000, 0.0);
        assert_eq!((m.po, m.swr, m.alc, m.id), (0, 0, 0, 0));
        // The supply is there whether or not it is transmitting.
        assert!(m.vd > 0);
    }

    #[test]
    fn a_transmitting_radio_shows_nothing_on_its_s_meter() {
        // And its current follows its power, because on a real radio the
        // current is what makes the power.
        let low = transmitting(64);
        let high = transmitting(255);
        assert_eq!(high.s, 0);
        assert!(high.po > low.po);
        assert!(high.id > low.id);
        // The supply sags under load.
        assert!(high.vd < receiving(&Band::empty(-130.0, 1), 14_100_000, 0.0).vd);
    }
}
