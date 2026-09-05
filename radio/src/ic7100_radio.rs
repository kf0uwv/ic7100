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

//! An IC-7100 answering CI-V.
//!
//! What the generic engine delegates to: one parsed command in, a state
//! change and a reply out. Framing, lookup and structural validation have
//! already happened — this is the part that knows what a command *means*.
//!
//! # A read answers with the command it was asked
//!
//! CI-V has no separate response codes. The radio replies with the same
//! `Cn [Sc]` and the data appended, so `03` (read frequency) is answered
//! by `03` carrying five BCD bytes. A write is answered by a bare `FB`,
//! and a refusal by `FA`. That is the whole reply vocabulary, and it is
//! why this file builds frames rather than formatting strings.

use cat_framework::civ::{decode_bcd, decode_bcd_be, encode_bcd, encode_bcd_be, CivFormat};
use cat_framework::{
    CatCommandCatalog, CatRadio, CommandOperation, CommandOutcome, CommandRequest, CommandTable,
    ProtocolErrorKind, ResponseBuilder,
};

use crate::command::{level, meter, Ic7100CommandId, FREQUENCY_BYTES, IC7100_COMMAND_TABLE};
use crate::mode::{Filter, Mode};
use crate::state::{Duplex, Ic7100State, Tuning, Vfo};

/// Something the radio did that a host might want to know about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ic7100Event {
    FrequencyChanged(u64),
    ModeChanged(Mode),
    VfoChanged(Vfo),
    TuningChanged(Tuning),
    SplitChanged(bool),
    TransmitChanged(bool),
    PowerChanged(bool),
}

/// What can go wrong inside the radio, as against in the framing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Ic7100Error {
    #[error("the response buffer was already finished")]
    ResponseAlreadyWritten,
}

/// An IC-7100.
#[derive(Debug, Clone)]
pub struct Ic7100Radio {
    state: Ic7100State,
    format: CivFormat,
}

impl Default for Ic7100Radio {
    fn default() -> Self {
        Self::new()
    }
}

impl Ic7100Radio {
    pub fn new() -> Self {
        Self {
            state: Ic7100State::default(),
            format: CivFormat::default(),
        }
    }

    /// A radio at a non-default CI-V address.
    ///
    /// The whole point of a bus: two IC-7100s differ only by this.
    pub fn at_address(address: u8) -> Self {
        let mut radio = Self::new();
        radio.state.civ_address = address;
        radio.format = CivFormat::for_radio(address);
        radio
    }

    pub fn state(&self) -> &Ic7100State {
        &self.state
    }

    /// Set what a meter reads.
    ///
    /// For an emulator whose meters follow the band it is transmitting.
    /// Named for the one thing rather than exposing the state, so a caller
    /// cannot move the dial from outside the command stream.
    pub fn set_meters(&mut self, meters: crate::state::Meters) {
        self.state.meters = meters;
    }

    fn format(&self) -> CivFormat {
        self.format
    }

    /// Answer with the same command code and some data.
    fn answer(
        &self,
        code: (u8, Option<u8>),
        data: &[u8],
        response: &mut ResponseBuilder<'_, CivFormat>,
    ) -> Result<CommandOutcome<Ic7100Event>, Ic7100Error> {
        response
            .write_frame(&self.format().encode_response(code, data))
            .map_err(|_| Ic7100Error::ResponseAlreadyWritten)?;
        Ok(CommandOutcome::response_written())
    }

    /// Acknowledge a write.
    fn ok(
        &self,
        response: &mut ResponseBuilder<'_, CivFormat>,
        events: Vec<Ic7100Event>,
    ) -> Result<CommandOutcome<Ic7100Event>, Ic7100Error> {
        response
            .write_frame(&self.format().encode_ok())
            .map_err(|_| Ic7100Error::ResponseAlreadyWritten)?;
        let mut outcome = CommandOutcome::response_written();
        outcome.events = events;
        Ok(outcome)
    }

    /// Refuse one.
    fn ng(
        &self,
        response: &mut ResponseBuilder<'_, CivFormat>,
    ) -> Result<CommandOutcome<Ic7100Event>, Ic7100Error> {
        response
            .write_frame(&self.format().encode_ng())
            .map_err(|_| Ic7100Error::ResponseAlreadyWritten)?;
        Ok(CommandOutcome::response_written())
    }

    /// The raw count a meter currently reads.
    fn meter_value(&self, sub: u8) -> Option<u8> {
        let m = &self.state.meters;
        Some(match sub {
            meter::S => m.s,
            meter::PO => m.po,
            meter::SWR => m.swr,
            meter::ALC => m.alc,
            meter::COMP => m.comp,
            meter::VD => m.vd,
            meter::ID => m.id,
            _ => return None,
        })
    }

    fn level_value(&self, sub: u8) -> Option<u8> {
        let l = &self.state.levels;
        Some(match sub {
            level::AF => l.af,
            level::RF_GAIN => l.rf_gain,
            level::SQUELCH => l.squelch,
            level::NR => l.noise_reduction,
            level::RF_POWER => l.rf_power,
            level::MIC_GAIN => l.mic_gain,
            level::CW_PITCH => l.cw_pitch,
            level::KEY_SPEED => l.key_speed,
            level::NOTCH => l.notch,
            _ => return None,
        })
    }

    fn set_level(&mut self, sub: u8, value: u8) -> bool {
        let l = &mut self.state.levels;
        match sub {
            level::AF => l.af = value,
            level::RF_GAIN => l.rf_gain = value,
            level::SQUELCH => l.squelch = value,
            level::NR => l.noise_reduction = value,
            level::RF_POWER => l.rf_power = value,
            level::MIC_GAIN => l.mic_gain = value,
            level::CW_PITCH => l.cw_pitch = value,
            level::KEY_SPEED => l.key_speed = value,
            level::NOTCH => l.notch = value,
            _ => return false,
        }
        true
    }
}

impl CatCommandCatalog<CivFormat> for Ic7100Radio {
    type CommandId = Ic7100CommandId;

    fn command_table(&self) -> &'static CommandTable<Self::CommandId, CivFormat> {
        &IC7100_COMMAND_TABLE
    }
}

impl CatRadio<CivFormat> for Ic7100Radio {
    type Event = Ic7100Event;
    type Error = Ic7100Error;

    fn handle_command(
        &mut self,
        request: CommandRequest<'_, Self::CommandId, CivFormat>,
        response: &mut ResponseBuilder<'_, CivFormat>,
    ) -> Result<CommandOutcome<Self::Event>, Self::Error> {
        use Ic7100CommandId as Id;
        let data = request.parameters.raw_bytes();
        let reading = request.operation == CommandOperation::Query || data.is_empty();

        match request.id {
            // 20-3, commands 03 and 05. Five BCD bytes either way.
            Id::ReadFrequency => {
                let hz = self.state.frequency();
                self.answer(request.code, &encode_bcd(hz, FREQUENCY_BYTES), response)
            }
            Id::SetFrequency | Id::SendFrequency => match decode_bcd(data) {
                // Refused rather than clamped. A frequency outside this
                // radio's coverage is a mistake somewhere upstream, and
                // moving to the nearest legal one would hide it while
                // leaving the operator somewhere they did not ask for.
                Some(hz) if crate::capabilities::covers(hz) => {
                    self.state.set_frequency(hz);
                    self.ok(response, vec![Ic7100Event::FrequencyChanged(hz)])
                }
                _ => self.ng(response),
            },

            // 20-3, commands 04 and 06. Mode byte, then an optional
            // filter byte -- the manual says it may be omitted.
            Id::ReadMode => {
                let data = [self.state.mode.as_u8(), self.state.filter as u8];
                self.answer(request.code, &data, response)
            }
            Id::SetMode | Id::SendMode => {
                let Some(mode) = data.first().copied().and_then(Mode::from_u8) else {
                    return self.ng(response);
                };
                self.state.mode = mode;
                if let Some(filter) = data.get(1) {
                    self.state.filter = match filter {
                        0x01 => Filter::Fil1,
                        0x02 => Filter::Fil2,
                        0x03 => Filter::Fil3,
                        _ => return self.ng(response),
                    };
                } else {
                    // "FIL1 is automatically selected with command 01."
                    self.state.filter = Filter::Fil1;
                }
                self.ok(response, vec![Ic7100Event::ModeChanged(mode)])
            }

            // 20-3, command 07.
            Id::SelectVfo => {
                let mut events = vec![Ic7100Event::TuningChanged(Tuning::Vfo)];
                self.state.tuning = Tuning::Vfo;
                match data.first() {
                    None => {}
                    Some(0x00) => {
                        self.state.selected = Vfo::A;
                        events.push(Ic7100Event::VfoChanged(Vfo::A));
                    }
                    Some(0x01) => {
                        self.state.selected = Vfo::B;
                        events.push(Ic7100Event::VfoChanged(Vfo::B));
                    }
                    // A0: equalise. B0: exchange.
                    Some(0xA0) => {
                        let f = self.state.frequency();
                        self.state.vfo_a_hz = f;
                        self.state.vfo_b_hz = f;
                    }
                    Some(0xB0) => {
                        std::mem::swap(&mut self.state.vfo_a_hz, &mut self.state.vfo_b_hz);
                    }
                    Some(_) => return self.ng(response),
                }
                self.ok(response, events)
            }

            // 20-3, command 08.
            Id::SelectMemory => {
                self.state.tuning = Tuning::Memory;
                if !data.is_empty() {
                    match decode_bcd_be(data) {
                        Some(ch) if (1..=99).contains(&ch) => self.state.memory_channel = ch as u8,
                        // 0100 and up are the scan edges and call
                        // channels, which are real on this radio but are
                        // not memories an operator picks by number.
                        _ => return self.ng(response),
                    }
                }
                self.ok(response, vec![Ic7100Event::TuningChanged(Tuning::Memory)])
            }
            Id::MemoryWrite | Id::MemoryToVfo | Id::MemoryClear => self.ok(response, Vec::new()),

            // 20-3, command 0E.
            Id::Scan => self.ok(response, Vec::new()),

            // 20-3, command 0F: split and duplex share one command.
            Id::SplitDuplex if reading => {
                let byte = match (self.state.split, self.state.duplex) {
                    (_, Duplex::Minus) => 0x11,
                    (_, Duplex::Plus) => 0x12,
                    (true, _) => 0x01,
                    (false, _) => 0x00,
                };
                self.answer(request.code, &[byte], response)
            }
            Id::SplitDuplex => match data.first() {
                Some(0x00) => {
                    self.state.split = false;
                    self.ok(response, vec![Ic7100Event::SplitChanged(false)])
                }
                Some(0x01) => {
                    self.state.split = true;
                    self.ok(response, vec![Ic7100Event::SplitChanged(true)])
                }
                Some(0x10) => {
                    self.state.duplex = Duplex::Simplex;
                    self.ok(response, Vec::new())
                }
                Some(0x11) => {
                    self.state.duplex = Duplex::Minus;
                    self.ok(response, Vec::new())
                }
                Some(0x12) => {
                    self.state.duplex = Duplex::Plus;
                    self.ok(response, Vec::new())
                }
                _ => self.ng(response),
            },

            // 20-3, command 10.
            Id::TuningStep if reading => {
                self.answer(request.code, &[self.state.tuning_step], response)
            }
            Id::TuningStep => match data.first() {
                Some(&step) if step <= 12 => {
                    self.state.tuning_step = step;
                    self.ok(response, Vec::new())
                }
                _ => self.ng(response),
            },

            // 20-3, command 11: only 00 and 12 exist on this radio.
            Id::Attenuator if reading => {
                self.answer(request.code, &[self.state.attenuator], response)
            }
            Id::Attenuator => match data.first() {
                Some(&v @ (0x00 | 0x12)) => {
                    self.state.attenuator = v;
                    self.ok(response, Vec::new())
                }
                _ => self.ng(response),
            },

            // 20-5, command 15 01 and 05.
            Id::SquelchStatus => {
                // Open when the signal clears the squelch setting. An
                // emulator with a real S-meter therefore has a real
                // squelch, which is what makes the two agree.
                let open = self.state.meters.s >= self.state.levels.squelch;
                self.answer(request.code, &[u8::from(open)], response)
            }

            // 20-5, command 15 xx: the seven meters. Two BCD bytes.
            Id::MeterS
            | Id::MeterPo
            | Id::MeterSwr
            | Id::MeterAlc
            | Id::MeterComp
            | Id::MeterVd
            | Id::MeterId => {
                let Some(sub) = request.code.1 else {
                    return self.ng(response);
                };
                match self.meter_value(sub) {
                    Some(raw) => {
                        self.answer(request.code, &encode_bcd_be(u64::from(raw), 2), response)
                    }
                    None => self.ng(response),
                }
            }

            // 20-4, command 14 xx: the levels. Two BCD bytes.
            Id::LevelAf
            | Id::LevelRfGain
            | Id::LevelSquelch
            | Id::LevelNoiseReduction
            | Id::LevelRfPower
            | Id::LevelMicGain
            | Id::LevelCwPitch
            | Id::LevelKeySpeed
            | Id::LevelNotch => {
                let Some(sub) = request.code.1 else {
                    return self.ng(response);
                };
                if reading {
                    return match self.level_value(sub) {
                        Some(raw) => {
                            self.answer(request.code, &encode_bcd_be(u64::from(raw), 2), response)
                        }
                        None => self.ng(response),
                    };
                }
                match decode_bcd_be(data) {
                    Some(v) if v <= 255 && self.set_level(sub, v as u8) => {
                        self.ok(response, Vec::new())
                    }
                    _ => self.ng(response),
                }
            }

            // 20-6, command 19 00.
            Id::ReadId => self.answer(request.code, &[self.state.civ_address], response),

            // 20-6, command 18.
            Id::Power => match data.first() {
                Some(0x00) => {
                    self.state.power_on = false;
                    self.ok(response, vec![Ic7100Event::PowerChanged(false)])
                }
                Some(0x01) => {
                    self.state.power_on = true;
                    self.ok(response, vec![Ic7100Event::PowerChanged(true)])
                }
                _ => self.ng(response),
            },

            // 1C 00: transmit. Reading it is how a console knows.
            Id::Transmit if reading => {
                self.answer(request.code, &[u8::from(self.state.transmitting)], response)
            }
            Id::Transmit => match data.first() {
                Some(&v @ (0x00 | 0x01)) => {
                    let on = v == 0x01;
                    // A mode this radio will not transmit in stays
                    // receiving. WFM is receive-only, and keying up in it
                    // is a request the radio itself refuses.
                    if on && !self.state.mode.can_transmit() {
                        return self.ng(response);
                    }
                    self.state.transmitting = on;
                    self.ok(response, vec![Ic7100Event::TransmitChanged(on)])
                }
                _ => self.ng(response),
            },

            Id::Function | Id::SetModeItem | Id::Tuner => self.ok(response, Vec::new()),
        }
    }

    fn write_protocol_error(
        &mut self,
        _kind: ProtocolErrorKind,
        response: &mut ResponseBuilder<'_, CivFormat>,
    ) -> Result<CommandOutcome<Self::Event>, Self::Error> {
        // CI-V has one refusal, `FA`, whatever went wrong. A radio does
        // not tell a controller *why* — which is a real limitation of the
        // protocol and not something to invent a richer answer for.
        self.ng(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cat_framework::wire_format::CatWireFormat;
    use cat_framework::CatFramework;

    fn radio() -> CatFramework<Ic7100Radio, CivFormat> {
        CatFramework::with_format(Ic7100Radio::new(), CivFormat::default())
    }

    /// Send one frame and get the radio's reply bytes.
    fn exchange(fw: &mut CatFramework<Ic7100Radio, CivFormat>, frame: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        fw.process_frame(frame, &mut out).expect("the frame parses");
        out
    }

    fn request(code: (u8, Option<u8>), data: &[u8]) -> Vec<u8> {
        CivFormat::default().encode_request(code, data)
    }

    #[test]
    fn reading_the_frequency_answers_with_the_same_command() {
        // CI-V has no response codes: a read of `03` is answered by `03`
        // with the data appended. A radio that replied with a different
        // code would be answering a question nobody asked.
        let mut fw = radio();
        let reply = exchange(&mut fw, &request((0x03, None), &[]));
        assert_eq!(reply[..5], [0xFE, 0xFE, 0xE0, 0x88, 0x03]);
        assert_eq!(decode_bcd(&reply[5..reply.len() - 1]), Some(14_100_000));
    }

    #[test]
    fn setting_the_frequency_moves_the_dial_and_is_acknowledged() {
        let mut fw = radio();
        let reply = exchange(
            &mut fw,
            &request((0x05, None), &encode_bcd(7_150_000, FREQUENCY_BYTES)),
        );
        assert_eq!(reply, CivFormat::default().encode_ok());
        assert_eq!(fw.radio().state().vfo_a_hz, 7_150_000);
    }

    #[test]
    fn a_frequency_this_radio_cannot_reach_is_refused_not_clamped() {
        // 300 MHz is in the gap between this radio's two receive ranges.
        // Clamping would leave an operator somewhere they did not ask for
        // and report success.
        let mut fw = radio();
        let before = fw.radio().state().vfo_a_hz;
        let reply = exchange(
            &mut fw,
            &request((0x05, None), &encode_bcd(300_000_000, FREQUENCY_BYTES)),
        );
        assert_eq!(reply, CivFormat::default().encode_ng());
        assert_eq!(fw.radio().state().vfo_a_hz, before, "the dial moved anyway");
    }

    #[test]
    fn a_mode_frame_without_a_filter_byte_selects_fil1() {
        // 20-11: "FIL1 is automatically selected with command 01."
        let mut fw = radio();
        exchange(&mut fw, &request((0x06, None), &[Mode::Cw.as_u8()]));
        assert_eq!(fw.radio().state().mode, Mode::Cw);
        assert_eq!(fw.radio().state().filter, Filter::Fil1);

        exchange(&mut fw, &request((0x06, None), &[Mode::Usb.as_u8(), 0x02]));
        assert_eq!(fw.radio().state().filter, Filter::Fil2);
    }

    #[test]
    fn every_meter_can_be_read_and_answers_its_own_value() {
        // Seven meters, seven distinct readings. A dispatcher that fell
        // through to one of them would report the S-meter for all of
        // them, and every reading would look plausible.
        let mut fw = radio();
        fw.radio_mut().set_meters(crate::state::Meters {
            s: 100,
            po: 90,
            swr: 80,
            alc: 70,
            comp: 60,
            vd: 50,
            id: 40,
        });
        for (sub, want) in [
            (meter::S, 100),
            (meter::PO, 90),
            (meter::SWR, 80),
            (meter::ALC, 70),
            (meter::COMP, 60),
            (meter::VD, 50),
            (meter::ID, 40),
        ] {
            let reply = exchange(&mut fw, &request((0x15, Some(sub)), &[]));
            let data = &reply[6..reply.len() - 1];
            assert_eq!(decode_bcd_be(data), Some(want), "sub-command {sub:#04X}");
        }
    }

    #[test]
    fn a_level_goes_on_the_wire_big_endian_and_a_frequency_does_not() {
        // The asymmetry, checked at the radio rather than only in the
        // codec: a level sent in the frequency's order is accepted and
        // sets something else, and 200 arrives as 2.
        let mut fw = radio();
        exchange(
            &mut fw,
            &request((0x14, Some(level::AF)), &encode_bcd_be(200, 2)),
        );
        assert_eq!(fw.radio().state().levels.af, 200);

        let reply = exchange(&mut fw, &request((0x14, Some(level::AF)), &[]));
        // `02 00`, not `00 02`.
        assert_eq!(&reply[6..reply.len() - 1], &[0x02, 0x00]);

        // And a frequency is the other way round: 14.074 MHz starts at
        // its 1 Hz end.
        exchange(
            &mut fw,
            &request((0x05, None), &encode_bcd(14_074_000, FREQUENCY_BYTES)),
        );
        let reply = exchange(&mut fw, &request((0x03, None), &[]));
        assert_eq!(&reply[5..reply.len() - 1], &[0x00, 0x40, 0x07, 0x14, 0x00]);
    }

    #[test]
    fn a_level_round_trips_through_the_radio() {
        let mut fw = radio();
        exchange(
            &mut fw,
            &request((0x14, Some(level::AF)), &encode_bcd_be(200, 2)),
        );
        let reply = exchange(&mut fw, &request((0x14, Some(level::AF)), &[]));
        assert_eq!(decode_bcd_be(&reply[6..reply.len() - 1]), Some(200));
        assert_eq!(fw.radio().state().levels.af, 200);
    }

    #[test]
    fn it_refuses_to_transmit_in_a_receive_only_mode() {
        // WFM is receive-only per the specification page. A radio that
        // keyed up anyway would be transmitting somewhere it must not.
        let mut fw = radio();
        exchange(&mut fw, &request((0x06, None), &[Mode::Wfm.as_u8()]));
        let reply = exchange(&mut fw, &request((0x1C, Some(0x00)), &[0x01]));
        assert_eq!(reply, CivFormat::default().encode_ng());
        assert!(!fw.radio().state().transmitting);
    }

    #[test]
    fn split_and_duplex_share_one_command_and_stay_distinct() {
        // 0F carries both. Collapsing them would have setting DUP+ turn
        // split on, which on a repeater band is a real mis-transmit.
        let mut fw = radio();
        exchange(&mut fw, &request((0x0F, None), &[0x01]));
        assert!(fw.radio().state().split);

        exchange(&mut fw, &request((0x0F, None), &[0x12]));
        assert_eq!(fw.radio().state().duplex, Duplex::Plus);
        assert!(fw.radio().state().split, "setting duplex cleared split");
    }

    #[test]
    fn exchanging_the_vfos_swaps_them() {
        let mut fw = radio();
        let (a, b) = {
            let s = fw.radio().state();
            (s.vfo_a_hz, s.vfo_b_hz)
        };
        exchange(&mut fw, &request((0x07, None), &[0xB0]));
        assert_eq!(fw.radio().state().vfo_a_hz, b);
        assert_eq!(fw.radio().state().vfo_b_hz, a);
    }

    #[test]
    fn a_memory_channel_beyond_the_regular_ninety_nine_is_refused() {
        // 0100 and up are scan edges and call channels -- real, but not
        // memories an operator picks by number, and the capability set
        // says 1-99.
        let mut fw = radio();
        assert_eq!(
            exchange(&mut fw, &request((0x08, None), &encode_bcd_be(100, 2))),
            CivFormat::default().encode_ng()
        );
        assert_eq!(
            exchange(&mut fw, &request((0x08, None), &encode_bcd_be(42, 2))),
            CivFormat::default().encode_ok()
        );
        assert_eq!(fw.radio().state().memory_channel, 42);
    }

    #[test]
    fn the_radio_reports_its_own_address() {
        // `19 00`, and it must be the address the radio is actually
        // answering on -- that is how a controller finds a radio whose
        // address it does not know.
        let mut fw =
            CatFramework::with_format(Ic7100Radio::at_address(0x94), CivFormat::for_radio(0x94));
        let reply = exchange(
            &mut fw,
            &CivFormat::for_radio(0x94).encode_request((0x19, Some(0x00)), &[]),
        );
        assert_eq!(reply[reply.len() - 2], 0x94);
    }

    #[test]
    fn the_squelch_opens_when_a_signal_clears_it() {
        // The S-meter and the squelch are the same radio's, so they agree.
        let mut fw = radio();
        exchange(
            &mut fw,
            &request((0x14, Some(level::SQUELCH)), &encode_bcd_be(100, 2)),
        );

        fw.radio_mut().set_meters(crate::state::Meters {
            s: 20,
            ..crate::state::Meters::receiving()
        });
        let closed = exchange(&mut fw, &request((0x15, Some(meter::SQUELCH_STATUS)), &[]));
        assert_eq!(closed[closed.len() - 2], 0x00);

        fw.radio_mut().set_meters(crate::state::Meters {
            s: 150,
            ..crate::state::Meters::receiving()
        });
        let open = exchange(&mut fw, &request((0x15, Some(meter::SQUELCH_STATUS)), &[]));
        assert_eq!(open[open.len() - 2], 0x01);
    }
}
