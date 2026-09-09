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

//! This radio, as Hamlib's rigctld sees it — for WSJT-X and everything
//! else that speaks it.
//!
//! # The mode names are this radio's problem
//!
//! `cat-rigctl` deliberately does not own a shared mode table, and the
//! IC-7100 is why that is right: it has `DV`, which Hamlib has no name
//! for, and `WFM`, which Hamlib calls `WFM` but this radio will not
//! transmit in. A shared table would have to be wrong for one radio to be
//! right for another.

use cat_rigctl::RigctlRadio;
use cat_transport_core::CatSession;
use radio::{Ic7100, Mode, RadioError};

/// A local newtype, so this crate has something of its own to attach the
/// impl to.
pub struct Ic7100Rigctl<S: CatSession>(pub Ic7100<S>);

impl<S: CatSession> Ic7100Rigctl<S> {
    pub fn new(radio: Ic7100<S>) -> Self {
        Self(radio)
    }
}

#[async_trait::async_trait(?Send)]
impl<S> RigctlRadio for Ic7100Rigctl<S>
where
    S: CatSession,
    S::Error: std::error::Error + 'static,
{
    type Mode = Mode;
    type Error = RadioError<S::Error>;

    fn unsupported() -> Self::Error {
        // This radio's CAT set has no such command; the closest honest
        // thing it can say is that it refused.
        RadioError::Refused
    }

    async fn get_vfo_a_hz(&mut self) -> Result<u64, Self::Error> {
        self.0.frequency().await
    }

    async fn set_vfo_a_hz(&mut self, hz: u64) -> Result<(), Self::Error> {
        self.0.set_frequency(hz).await
    }

    async fn get_mode(&mut self) -> Result<Self::Mode, Self::Error> {
        self.0.mode().await.map(|(mode, _filter)| mode)
    }

    async fn set_mode(&mut self, mode: Self::Mode) -> Result<(), Self::Error> {
        self.0.set_mode(mode).await
    }

    async fn get_transmitting(&mut self) -> Result<bool, Self::Error> {
        // `1C 00`. Not inferred from a meter: a radio keyed from its own
        // front panel is transmitting, and a PO reading of zero on a
        // carrier-less mode would say otherwise.
        self.0.transmitting().await
    }

    async fn transmit(&mut self) -> Result<(), Self::Error> {
        self.0.set_transmit(true).await
    }

    async fn receive(&mut self) -> Result<(), Self::Error> {
        self.0.set_transmit(false).await
    }

    /// Hamlib's name for one of this radio's modes.
    ///
    /// `DV` has no Hamlib counterpart at all. Reported as `PKTFM`, the
    /// closest thing Hamlib has to "a digital mode on an FM carrier" —
    /// which is what D-STAR is — rather than as `FM`, which would have
    /// WSJT-X believe it could work analogue FM through it.
    fn hamlib_mode_name(mode: Self::Mode) -> &'static str {
        match mode {
            Mode::Lsb => "LSB",
            Mode::Usb => "USB",
            Mode::Am => "AM",
            Mode::Cw => "CW",
            Mode::CwReverse => "CWR",
            Mode::Rtty => "RTTY",
            Mode::RttyReverse => "RTTYR",
            Mode::Fm => "FM",
            Mode::Wfm => "WFM",
            Mode::Dv => "PKTFM",
        }
    }

    /// What `\dump_state` reports as this radio's range.
    ///
    /// The outer envelope, 30 kHz to 470 MHz, because Hamlib's format
    /// carries one range per row and this radio's coverage has a hole in
    /// it between 200 and 400 MHz. Over-claiming there is the lesser
    /// error: the radio itself refuses a frequency it cannot reach, so a
    /// client that trusts the row gets a clean refusal rather than
    /// silence.
    /// This radio describes itself, so `\dump_state`'s capability tail is
    /// generated rather than hand-written — which is where a field-count
    /// bug once made Hamlib block forever with nothing pointing at why.
    fn capabilities() -> Option<&'static cat_framework::capabilities::RadioCapabilities> {
        Some(&radio::capabilities::IC7100)
    }

    fn freq_range_hz() -> (u64, u64) {
        let range = radio::capabilities::IC7100.rx_range;
        (range.min_hz, range.max_hz)
    }

    /// Split, which this radio carries on the same command as duplex.
    ///
    /// `RigctlRadio`'s default refuses, and this impl inherited that
    /// refusal while `Ic7100::split_and_duplex` sat one call away. So
    /// rigctl's `s` reported "not split" on a radio that was in split, and
    /// `S 1` refused outright -- a defaulted trait method nobody overrode
    /// looks exactly like a radio that cannot do the thing.
    async fn get_split(&mut self) -> Result<bool, Self::Error> {
        // The duplex half of the answer is dropped: rigctl asks a yes/no
        // question here, and `Duplex::Plus` is repeater shift, not split.
        self.0
            .split_and_duplex()
            .await
            .map(|(split, _duplex)| split)
    }

    async fn set_split(&mut self, on: bool) -> Result<(), Self::Error> {
        self.0.set_split(on).await
    }

    /// The `ModeId` -> `Mode` crossing, so a cached poll can answer `m`.
    ///
    /// Without it every mode read goes to the wire. Not wrong -- the
    /// cache falls through rather than guessing -- but it spends the
    /// shared CI-V link on a question already answered.
    fn mode_from_id(id: cat_framework::capabilities::ModeId) -> Option<Self::Mode> {
        Mode::from_shared(id)
    }

    fn hamlib_mode_from_name(name: &str) -> Option<Self::Mode> {
        Some(match name {
            "LSB" => Mode::Lsb,
            "USB" => Mode::Usb,
            "AM" => Mode::Am,
            "CW" => Mode::Cw,
            "CWR" => Mode::CwReverse,
            "RTTY" => Mode::Rtty,
            "RTTYR" => Mode::RttyReverse,
            "FM" => Mode::Fm,
            "WFM" => Mode::Wfm,
            "PKTFM" => Mode::Dv,
            // `PKTUSB` is what WSJT-X asks for on HF data. This radio has
            // no separate data mode -- it works FT8 in USB -- so that is
            // what it gets, rather than a refusal the operator would have
            // to diagnose.
            "PKTUSB" | "DATA-U" => Mode::Usb,
            "PKTLSB" | "DATA-L" => Mode::Lsb,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Nothing;

    #[async_trait::async_trait(?Send)]
    impl CatSession for Nothing {
        type Error = cat_transport_core::TransportError;
        async fn execute(
            &mut self,
            _r: &[u8],
            _o: &mut Vec<u8>,
        ) -> Result<cat_framework::ResponseDisposition, Self::Error> {
            unreachable!("these tests only exercise the name mapping")
        }
    }

    type R = Ic7100Rigctl<Nothing>;

    #[test]
    fn every_mode_this_radio_can_report_crosses_back_from_its_shared_id() {
        // `mode_from_id` is what lets a cached poll answer `m` without
        // going to the wire. It was never overridden, so it returned
        // `None` for everything and every mode read went out over CI-V.
        for descriptor in radio::capabilities::IC7100.modes.iter() {
            assert!(
                R::mode_from_id(descriptor.id).is_some(),
                "{} is declared but does not cross back from its ModeId",
                descriptor.label
            );
        }
    }

    /// A session whose other end is this radio's own emulator, so a
    /// command written comes back read.
    struct Loopback {
        radio: cat_framework::CatFramework<radio::Ic7100Radio, cat_framework::civ::CivFormat>,
    }

    #[async_trait::async_trait(?Send)]
    impl CatSession for Loopback {
        type Error = cat_transport_core::TransportError;
        async fn execute(
            &mut self,
            request: &[u8],
            response: &mut Vec<u8>,
        ) -> Result<cat_framework::ResponseDisposition, Self::Error> {
            self.radio
                .process_frame(request, response)
                .map(|o| o.response)
                .map_err(|_| cat_transport_core::TransportError::Other("frame refused".to_string()))
        }
    }

    fn emulated() -> Ic7100Rigctl<Loopback> {
        Ic7100Rigctl::new(radio::Ic7100::new(Loopback {
            radio: cat_framework::CatFramework::with_format(
                radio::Ic7100Radio::new(),
                cat_framework::civ::CivFormat::default(),
            ),
        }))
    }

    #[monoio::test(driver = "legacy")]
    async fn split_survives_the_round_trip_through_the_bridge() {
        // `get_split`/`set_split` are defaulted on `RigctlRadio`, and this
        // impl inherited the default: rigctl's `s` answered "not split" on
        // a radio that was in split, and `S 1` refused, while command 0F
        // sat one call away. This asserts the round trip rather than the
        // methods' presence, so deleting either impl fails the test
        // instead of quietly compiling.
        let mut r = emulated();
        assert!(!r.get_split().await.expect("split is readable"));
        r.set_split(true).await.expect("split is settable");
        assert!(r.get_split().await.expect("split is readable"));
        r.set_split(false).await.expect("split is settable");
        assert!(!r.get_split().await.expect("split is readable"));
    }

    #[test]
    fn every_mode_this_radio_has_gets_a_hamlib_name() {
        for mode in [
            Mode::Lsb,
            Mode::Usb,
            Mode::Am,
            Mode::Cw,
            Mode::CwReverse,
            Mode::Rtty,
            Mode::RttyReverse,
            Mode::Fm,
            Mode::Wfm,
            Mode::Dv,
        ] {
            let name = R::hamlib_mode_name(mode);
            assert!(!name.is_empty(), "{mode:?}");
        }
    }

    #[test]
    fn the_names_round_trip_for_everything_hamlib_can_say() {
        // `PKTUSB` and friends map *into* this radio without mapping back
        // out, which is deliberate: this radio has no separate data mode.
        for mode in [
            Mode::Lsb,
            Mode::Usb,
            Mode::Am,
            Mode::Cw,
            Mode::CwReverse,
            Mode::Rtty,
            Mode::RttyReverse,
            Mode::Fm,
            Mode::Wfm,
            Mode::Dv,
        ] {
            let name = R::hamlib_mode_name(mode);
            assert_eq!(R::hamlib_mode_from_name(name), Some(mode), "{name}");
        }
    }

    #[test]
    fn dv_is_not_reported_as_plain_fm() {
        // Reporting D-STAR as FM would have WSJT-X believe it could work
        // analogue modes through a digital-voice channel.
        assert_ne!(R::hamlib_mode_name(Mode::Dv), R::hamlib_mode_name(Mode::Fm));
    }

    #[test]
    fn wsjtx_data_mode_requests_land_somewhere_that_works() {
        // WSJT-X asks for PKTUSB on HF. This radio works FT8 in USB, so
        // that is what it gets -- a refusal here is an operator staring at
        // a rig-control error for a mode their radio does not need.
        assert_eq!(R::hamlib_mode_from_name("PKTUSB"), Some(Mode::Usb));
        assert_eq!(R::hamlib_mode_from_name("PKTLSB"), Some(Mode::Lsb));
    }

    #[test]
    fn a_name_this_radio_has_no_mode_for_is_refused() {
        assert_eq!(R::hamlib_mode_from_name("C4FM"), None);
        assert_eq!(R::hamlib_mode_from_name("nonsense"), None);
    }
}
