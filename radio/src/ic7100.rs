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

//! Talking to an IC-7100.
//!
//! Typed methods over `cat-client`'s generic request/response mechanics.
//! Generic over the session, so this crate never names a transport — the
//! application's wiring layer chooses one.
//!
//! # Why the replies are taken apart here
//!
//! CI-V answers a read with the command it was asked, wrapped in the same
//! envelope as everything else. So every method below has to strip
//! `FE FE <to> <from> Cn [Sc]` back off before it can read the data. That
//! is done once, in [`Ic7100::payload_of`], rather than at each call site
//! — twenty methods each slicing an envelope by hand is twenty chances to
//! be off by the length of a sub-command byte.

use cat_client::{CatClient, ClientError};
use cat_framework::civ::{decode_bcd, decode_bcd_be, encode_bcd, encode_bcd_be, CivFormat};
use cat_transport_core::CatSession;

use crate::command::{level, meter, Ic7100CommandId, FREQUENCY_BYTES, IC7100_COMMAND_TABLE};
use crate::mode::{Filter, Mode};
use crate::state::{Duplex, Meters};

/// What can go wrong talking to this radio.
#[derive(Debug, thiserror::Error)]
pub enum RadioError<E: std::error::Error + 'static> {
    #[error("the radio refused the command")]
    Refused,
    #[error("the radio answered with a frame that is not a reply to this command")]
    UnexpectedReply,
    #[error("the radio reported a mode this build does not know: {0:#04X}")]
    UnknownMode(u8),
    #[error("{0} Hz is outside this radio's coverage")]
    OutOfCoverage(u64),
    #[error(transparent)]
    Client(#[from] ClientError<E>),
}

pub type RadioResult<T, E> = Result<T, RadioError<E>>;

/// An IC-7100 at the other end of a session.
pub struct Ic7100<S: CatSession> {
    client: CatClient<Ic7100CommandId, S, CivFormat>,
    format: CivFormat,
}

impl<S> Ic7100<S>
where
    S: CatSession,
    S::Error: std::error::Error + 'static,
{
    /// A radio at the factory address.
    pub fn new(session: S) -> Self {
        Self::at_address(session, CivFormat::default().radio)
    }

    /// A radio at `address` — the reason CI-V is a bus.
    pub fn at_address(session: S, address: u8) -> Self {
        let format = CivFormat::for_radio(address);
        Self {
            client: CatClient::with_format(session, &IC7100_COMMAND_TABLE, format),
            format,
        }
    }

    /// Point this client at a different radio on the bus.
    ///
    /// For a controller sweeping addresses to find what is out there:
    /// opening a serial port per address would open two hundred of them
    /// to ask one question each, and on a real port that is slow enough
    /// to look like a hang.
    pub fn set_address(&mut self, address: u8) {
        self.format = CivFormat::for_radio(address);
        self.client.set_format(self.format);
    }

    /// The address this client is currently talking to.
    pub fn address(&self) -> u8 {
        self.format.radio
    }

    /// The data after the envelope and the command code.
    ///
    /// Rejects a frame that is not this radio's answer to this command:
    /// on a shared bus another radio's traffic arrives here too, and a
    /// method that read it would report a second radio's dial as its own.
    fn payload_of<'a>(
        &self,
        frame: &'a [u8],
        code: (u8, Option<u8>),
    ) -> Result<&'a [u8], RadioError<S::Error>> {
        // A bare `FA` is the radio saying no.
        if let Some(env) = cat_framework::civ::parse_envelope(frame) {
            if env.payload == [cat_framework::civ::NG] {
                return Err(RadioError::Refused);
            }
        }
        if !self.format.is_for_us(frame) {
            return Err(RadioError::UnexpectedReply);
        }
        let env = cat_framework::civ::parse_envelope(frame).ok_or(RadioError::UnexpectedReply)?;
        let mut rest = env.payload;
        if rest.first() != Some(&code.0) {
            return Err(RadioError::UnexpectedReply);
        }
        rest = &rest[1..];
        if let Some(sub) = code.1 {
            if rest.first() != Some(&sub) {
                return Err(RadioError::UnexpectedReply);
            }
            rest = &rest[1..];
        }
        Ok(rest)
    }

    /// Whether a reply was the radio's acknowledgement.
    fn accepted(&self, frame: &[u8]) -> Result<(), RadioError<S::Error>> {
        match cat_framework::civ::parse_envelope(frame).map(|e| e.payload) {
            Some([cat_framework::civ::OK]) => Ok(()),
            Some([cat_framework::civ::NG]) => Err(RadioError::Refused),
            // Some writes are answered by the command itself rather than
            // a bare OK. Either is acceptance; only `FA` is refusal.
            Some(_) => Ok(()),
            None => Err(RadioError::UnexpectedReply),
        }
    }

    // -- frequency ------------------------------------------------------

    /// The operating frequency. Manual 20-3, command 03.
    pub async fn frequency(&mut self) -> RadioResult<u64, S::Error> {
        let code = (0x03, None);
        let reply = self.client.query_bytes(code).await?;
        let data = self.payload_of(&reply, code)?;
        decode_bcd(data).ok_or(RadioError::UnexpectedReply)
    }

    /// Set it. Manual 20-3, command 05.
    pub async fn set_frequency(&mut self, hz: u64) -> RadioResult<(), S::Error> {
        // Checked here as well as by the radio, so a console gets a reason
        // rather than a bare `FA` it would have to guess about.
        if !crate::capabilities::covers(hz) {
            return Err(RadioError::OutOfCoverage(hz));
        }
        let reply = self
            .client
            .set_bytes((0x05, None), &encode_bcd(hz, FREQUENCY_BYTES))
            .await?;
        self.accepted(&reply)
    }

    // -- mode -----------------------------------------------------------

    /// The operating mode and filter. Manual 20-3, command 04.
    pub async fn mode(&mut self) -> RadioResult<(Mode, Filter), S::Error> {
        let code = (0x04, None);
        let reply = self.client.query_bytes(code).await?;
        let data = self.payload_of(&reply, code)?;
        let byte = *data.first().ok_or(RadioError::UnexpectedReply)?;
        let mode = Mode::from_u8(byte).ok_or(RadioError::UnknownMode(byte))?;
        let filter = match data.get(1) {
            Some(0x02) => Filter::Fil2,
            Some(0x03) => Filter::Fil3,
            // Absent or 01. The manual says the filter byte may be
            // omitted, in which case FIL1 is in force.
            _ => Filter::Fil1,
        };
        Ok((mode, filter))
    }

    /// Set it. Manual 20-3, command 06.
    pub async fn set_mode(&mut self, mode: Mode) -> RadioResult<(), S::Error> {
        let reply = self.client.set_bytes((0x06, None), &[mode.as_u8()]).await?;
        self.accepted(&reply)
    }

    /// Set the mode and choose the filter with it.
    pub async fn set_mode_with_filter(
        &mut self,
        mode: Mode,
        filter: Filter,
    ) -> RadioResult<(), S::Error> {
        let reply = self
            .client
            .set_bytes((0x06, None), &[mode.as_u8(), filter as u8])
            .await?;
        self.accepted(&reply)
    }

    // -- meters ---------------------------------------------------------

    /// One meter's raw reading. Manual 20-5, command 15.
    pub async fn meter(&mut self, sub: u8) -> RadioResult<u8, S::Error> {
        let code = (0x15, Some(sub));
        let reply = self.client.query_bytes(code).await?;
        let data = self.payload_of(&reply, code)?;
        let raw = decode_bcd_be(data).ok_or(RadioError::UnexpectedReply)?;
        u8::try_from(raw).map_err(|_| RadioError::UnexpectedReply)
    }

    /// Every meter, in one pass.
    ///
    /// Seven round trips, because CI-V has no command that reads them
    /// together. Grouped here rather than at the call site so a console
    /// gets one snapshot and cannot show an S-meter from one moment beside
    /// an SWR from another.
    pub async fn meters(&mut self) -> RadioResult<Meters, S::Error> {
        Ok(Meters {
            s: self.meter(meter::S).await?,
            po: self.meter(meter::PO).await?,
            swr: self.meter(meter::SWR).await?,
            alc: self.meter(meter::ALC).await?,
            comp: self.meter(meter::COMP).await?,
            vd: self.meter(meter::VD).await?,
            id: self.meter(meter::ID).await?,
        })
    }

    /// The S-meter alone, for a console polling at readout rate.
    pub async fn smeter(&mut self) -> RadioResult<u8, S::Error> {
        self.meter(meter::S).await
    }

    // -- levels ---------------------------------------------------------

    /// One level's raw value. Manual 20-4, command 14.
    pub async fn level(&mut self, sub: u8) -> RadioResult<u8, S::Error> {
        let code = (0x14, Some(sub));
        let reply = self.client.query_bytes(code).await?;
        let data = self.payload_of(&reply, code)?;
        let raw = decode_bcd_be(data).ok_or(RadioError::UnexpectedReply)?;
        u8::try_from(raw).map_err(|_| RadioError::UnexpectedReply)
    }

    /// Set one. Big-endian BCD — see `cat_framework::civ`.
    pub async fn set_level(&mut self, sub: u8, value: u8) -> RadioResult<(), S::Error> {
        let reply = self
            .client
            .set_bytes((0x14, Some(sub)), &encode_bcd_be(u64::from(value), 2))
            .await?;
        self.accepted(&reply)
    }

    pub async fn af_gain(&mut self) -> RadioResult<u8, S::Error> {
        self.level(level::AF).await
    }

    pub async fn set_af_gain(&mut self, value: u8) -> RadioResult<(), S::Error> {
        self.set_level(level::AF, value).await
    }

    pub async fn rf_gain(&mut self) -> RadioResult<u8, S::Error> {
        self.level(level::RF_GAIN).await
    }

    pub async fn squelch(&mut self) -> RadioResult<u8, S::Error> {
        self.level(level::SQUELCH).await
    }

    pub async fn rf_power(&mut self) -> RadioResult<u8, S::Error> {
        self.level(level::RF_POWER).await
    }

    // -- split, duplex, VFO ---------------------------------------------

    /// Split and duplex, which this radio carries on one command.
    /// Manual 20-3, command 0F.
    pub async fn split_and_duplex(&mut self) -> RadioResult<(bool, Duplex), S::Error> {
        let code = (0x0F, None);
        let reply = self.client.query_bytes(code).await?;
        let data = self.payload_of(&reply, code)?;
        Ok(match data.first() {
            Some(0x00) => (false, Duplex::Simplex),
            Some(0x01) => (true, Duplex::Simplex),
            Some(0x11) => (false, Duplex::Minus),
            Some(0x12) => (false, Duplex::Plus),
            _ => return Err(RadioError::UnexpectedReply),
        })
    }

    pub async fn set_split(&mut self, on: bool) -> RadioResult<(), S::Error> {
        let reply = self.client.set_bytes((0x0F, None), &[u8::from(on)]).await?;
        self.accepted(&reply)
    }

    /// Select VFO A or B. Manual 20-3, command 07.
    pub async fn select_vfo(&mut self, vfo: crate::state::Vfo) -> RadioResult<(), S::Error> {
        let byte = match vfo {
            crate::state::Vfo::A => 0x00,
            crate::state::Vfo::B => 0x01,
        };
        let reply = self.client.set_bytes((0x07, None), &[byte]).await?;
        self.accepted(&reply)
    }

    /// Recall a memory channel. Manual 20-3, command 08.
    pub async fn select_memory(&mut self, channel: u8) -> RadioResult<(), S::Error> {
        let reply = self
            .client
            .set_bytes((0x08, None), &encode_bcd_be(u64::from(channel), 2))
            .await?;
        self.accepted(&reply)
    }

    // -- transmit -------------------------------------------------------

    /// Whether the radio is transmitting. Manual, command 1C 00.
    ///
    /// Read rather than inferred from a meter: a radio keyed from its own
    /// front panel is transmitting, and a PO reading of zero on a
    /// carrier-less mode would say otherwise.
    pub async fn transmitting(&mut self) -> RadioResult<bool, S::Error> {
        let code = (0x1C, Some(0x00));
        let reply = self.client.query_bytes(code).await?;
        let data = self.payload_of(&reply, code)?;
        Ok(data.first() == Some(&0x01))
    }

    /// Key or unkey it.
    ///
    /// The radio refuses in a receive-only mode, and that refusal is
    /// reported rather than swallowed — an operator whose transmit did
    /// nothing needs to know it did nothing.
    pub async fn set_transmit(&mut self, on: bool) -> RadioResult<(), S::Error> {
        let reply = self
            .client
            .set_bytes((0x1C, Some(0x00)), &[u8::from(on)])
            .await?;
        self.accepted(&reply)
    }

    // -- identity -------------------------------------------------------

    /// The address this radio is answering on. Manual 20-6, command 19 00.
    ///
    /// How a controller finds a radio whose address it does not know,
    /// which on a bus is the first thing it needs.
    pub async fn civ_address(&mut self) -> RadioResult<u8, S::Error> {
        let code = (0x19, Some(0x00));
        let reply = self.client.query_bytes(code).await?;
        let data = self.payload_of(&reply, code)?;
        data.first().copied().ok_or(RadioError::UnexpectedReply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cat_framework::wire_format::CatWireFormat;
    use cat_framework::{CatFramework, ResponseDisposition};
    use cat_transport_core::TransportError;

    /// A session wired straight to a radio state machine.
    ///
    /// The radio itself rather than a script of canned replies: a script
    /// asserts what the test author believed the radio says, and this
    /// asserts what it actually says.
    struct Loopback {
        radio: CatFramework<crate::Ic7100Radio, CivFormat>,
    }

    impl Loopback {
        fn new() -> Self {
            Self {
                radio: CatFramework::with_format(crate::Ic7100Radio::new(), CivFormat::default()),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl CatSession for Loopback {
        type Error = TransportError;

        async fn execute(
            &mut self,
            request: &[u8],
            response: &mut Vec<u8>,
        ) -> Result<ResponseDisposition, Self::Error> {
            self.radio
                .process_frame(request, response)
                .map(|o| o.response)
                .map_err(|_| TransportError::Other("the radio refused the frame".to_string()))
        }
    }

    fn radio() -> Ic7100<Loopback> {
        Ic7100::new(Loopback::new())
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        futures::executor::block_on(f)
    }

    #[test]
    fn a_frequency_survives_the_round_trip() {
        block_on(async {
            let mut r = radio();
            r.set_frequency(7_150_000).await.unwrap();
            assert_eq!(r.frequency().await.unwrap(), 7_150_000);
        });
    }

    #[test]
    fn a_frequency_outside_coverage_is_named_rather_than_refused_blankly() {
        // The radio would answer `FA`, which tells a console nothing. This
        // says which frequency and why, which is what an operator needs.
        block_on(async {
            let mut r = radio();
            match r.set_frequency(300_000_000).await {
                Err(RadioError::OutOfCoverage(hz)) => assert_eq!(hz, 300_000_000),
                other => panic!("expected a coverage error, got {other:?}"),
            }
        });
    }

    #[test]
    fn a_mode_survives_the_round_trip_with_its_filter() {
        block_on(async {
            let mut r = radio();
            r.set_mode_with_filter(Mode::Cw, Filter::Fil2)
                .await
                .unwrap();
            assert_eq!(r.mode().await.unwrap(), (Mode::Cw, Filter::Fil2));

            // Setting the mode alone leaves FIL1, as the manual says.
            r.set_mode(Mode::Usb).await.unwrap();
            assert_eq!(r.mode().await.unwrap(), (Mode::Usb, Filter::Fil1));
        });
    }

    #[test]
    fn all_seven_meters_read_back_distinctly() {
        // A dispatcher that fell through would report one meter for all
        // seven, and every reading would look plausible.
        block_on(async {
            let mut session = Loopback::new();
            session.radio.radio_mut().set_meters(Meters {
                s: 120,
                po: 100,
                swr: 48,
                alc: 60,
                comp: 130,
                vd: 18,
                id: 97,
            });
            let mut r = Ic7100::new(session);
            let m = r.meters().await.unwrap();
            assert_eq!(
                (m.s, m.po, m.swr, m.alc, m.comp, m.vd, m.id),
                (120, 100, 48, 60, 130, 18, 97)
            );
        });
    }

    #[test]
    fn a_level_survives_the_round_trip_in_the_right_byte_order() {
        // 200 sent little-endian would come back as 2. The client and the
        // radio have to agree, and both have to agree with the manual.
        block_on(async {
            let mut r = radio();
            r.set_af_gain(200).await.unwrap();
            assert_eq!(r.af_gain().await.unwrap(), 200);
        });
    }

    #[test]
    fn another_radios_traffic_is_not_read_as_ours() {
        // The bus. A reply from a radio at a different address must not be
        // taken for this one's, or a console shows a second radio's dial.
        let ours = Ic7100::<Loopback>::new(Loopback::new());
        let theirs =
            CivFormat::for_radio(0x94).encode_response((0x03, None), &encode_bcd(21_000_000, 5));
        assert!(matches!(
            ours.payload_of(&theirs, (0x03, None)),
            Err(RadioError::UnexpectedReply)
        ));
    }

    #[test]
    fn our_own_echo_is_not_read_as_a_reply() {
        // On a single-wire bus the request comes back first. Reading it as
        // the answer would have every value a console displayed be the one
        // it had just asked for.
        let ours = Ic7100::<Loopback>::new(Loopback::new());
        let echo = CivFormat::default().encode_request((0x03, None), &[]);
        assert!(matches!(
            ours.payload_of(&echo, (0x03, None)),
            Err(RadioError::UnexpectedReply)
        ));
    }

    #[test]
    fn a_refusal_is_reported_as_one() {
        let ours = Ic7100::<Loopback>::new(Loopback::new());
        let ng = CivFormat::default().encode_ng();
        assert!(matches!(
            ours.payload_of(&ng, (0x05, None)),
            Err(RadioError::Refused)
        ));
    }

    #[test]
    fn split_and_duplex_read_back_separately() {
        block_on(async {
            let mut r = radio();
            r.set_split(true).await.unwrap();
            assert_eq!(r.split_and_duplex().await.unwrap(), (true, Duplex::Simplex));
        });
    }

    #[test]
    fn the_radio_reports_the_address_it_answers_on() {
        block_on(async {
            let mut r = radio();
            assert_eq!(r.civ_address().await.unwrap(), 0x88);
        });
    }
}
