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

//! This radio's CI-V commands.
//!
//! Every entry cites the page of the IC-7100 Full Manual it came from. A
//! command byte that is wrong selects a *different real command* — CI-V
//! has no names, only numbers — so an error here does not fail, it does
//! something else. That is why the citations are not decoration.
//!
//! # What is here and what is not
//!
//! The manual's table runs to eight pages and several hundred
//! sub-commands, most of them set-mode items reached through `1A 05 nnnn`.
//! This is the set a console operates a radio with: frequency, mode, VFO
//! and memory, the seven meters, the levels, and the receiver switches.
//! The set-mode space is reachable through one command rather than
//! transcribed item by item, because transcribing it is how a table drifts
//! from the radio.

use cat_framework::civ::CivFormat;
use cat_framework::{CommandDefinition, CommandForm, CommandOperation, CommandTable};

/// A command, as this radio's code refers to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ic7100CommandId {
    /// 00 — send the operating frequency (transceive).
    SendFrequency,
    /// 01 — send the operating mode (transceive).
    SendMode,
    /// 03 — read the operating frequency.
    ReadFrequency,
    /// 04 — read the operating mode.
    ReadMode,
    /// 05 — set the operating frequency.
    SetFrequency,
    /// 06 — set the operating mode.
    SetMode,
    /// 07 — select the VFO mode, and the VFO within it.
    SelectVfo,
    /// 08 — select memory mode, or a memory channel.
    SelectMemory,
    /// 09 — write the current state to the selected memory.
    MemoryWrite,
    /// 0A — copy the selected memory to the VFO.
    MemoryToVfo,
    /// 0B — clear the selected memory.
    MemoryClear,
    /// 0E — scan.
    Scan,
    /// 0F — split and duplex.
    SplitDuplex,
    /// 10 — the tuning step.
    TuningStep,
    /// 11 — the attenuator.
    Attenuator,
    /// 15 — the squelch status.
    SquelchStatus,
    /// 15 {S} — read the s-meter level.
    MeterS,
    /// 15 {PO} — read the po meter level.
    MeterPo,
    /// 15 {SWR} — read the swr meter level.
    MeterSwr,
    /// 15 {ALC} — read the alc meter level.
    MeterAlc,
    /// 15 {COMP} — read the comp meter level.
    MeterComp,
    /// 15 {VD} — read the vd (supply voltage) meter level.
    MeterVd,
    /// 15 {ID} — read the id (drain current) meter level.
    MeterId,
    /// 14 {AF} — read or set the AF level.
    LevelAf,
    /// 14 {RF_GAIN} — read or set the RF gain.
    LevelRfGain,
    /// 14 {SQUELCH} — read or set the squelch level.
    LevelSquelch,
    /// 14 {NR} — read or set the NR level.
    LevelNoiseReduction,
    /// 14 {RF_POWER} — read or set the RF power level.
    LevelRfPower,
    /// 14 {MIC_GAIN} — read or set the microphone gain.
    LevelMicGain,
    /// 14 {CW_PITCH} — read or set the CW pitch.
    LevelCwPitch,
    /// 14 {KEY_SPEED} — read or set the keyer speed.
    LevelKeySpeed,
    /// 14 {NOTCH} — read or set the manual notch position.
    LevelNotch,
    /// 16 — the receiver and transmitter switches.
    Function,
    /// 18 — power off, and power on.
    Power,
    /// 19 — read the transceiver's own address.
    ReadId,
    /// 1A 05 — the set mode: one item per four-digit index.
    SetModeItem,
    /// 1C 00 — the transmit/receive state, read or set.
    Transmit,
    /// 1C 01 — the antenna tuner.
    Tuner,
}

/// One sub-command of `14`, the level group. Manual 20-4.
pub mod level {
    pub const AF: u8 = 0x01;
    pub const RF_GAIN: u8 = 0x02;
    pub const SQUELCH: u8 = 0x03;
    pub const NR: u8 = 0x06;
    /// The inner [TWIN PBT] control. 0000 cuts the higher passband edge,
    /// 0128 is centre, 0255 cuts the lower.
    pub const PBT_INNER: u8 = 0x07;
    pub const PBT_OUTER: u8 = 0x08;
    /// 0000 = 300 Hz, 0128 = 600 Hz, 0255 = 900 Hz.
    pub const CW_PITCH: u8 = 0x09;
    pub const RF_POWER: u8 = 0x0A;
    pub const MIC_GAIN: u8 = 0x0B;
    /// 0000 = 6 WPM, 0255 = 48 WPM.
    pub const KEY_SPEED: u8 = 0x0C;
    /// 0000 lowest, 0128 centre, 0255 highest.
    pub const NOTCH: u8 = 0x0D;
    pub const COMP: u8 = 0x0E;
    pub const BREAK_IN_DELAY: u8 = 0x0F;
    pub const LCD_CONTRAST: u8 = 0x18;
    pub const LCD_BACKLIGHT: u8 = 0x19;
}

/// One sub-command of `15`, the meter group. Manual 20-5.
///
/// The calibration for each is in `capabilities.rs` beside the
/// `MeterDescriptor` it belongs to.
pub mod meter {
    /// 00 closed, 01 open.
    pub const SQUELCH_STATUS: u8 = 0x01;
    /// 0000 = S0, 0120 = S9, 0241 = S9+60 dB.
    pub const S: u8 = 0x02;
    /// Various SQL function status.
    pub const SQL_STATUS: u8 = 0x05;
    /// 0000 = 0%, 0143 = 50%, 0213 = 100%.
    pub const PO: u8 = 0x11;
    /// 0000 = SWR 1.0, 0048 = 1.5, 0080 = 2.0, 0120 = 3.0.
    pub const SWR: u8 = 0x12;
    /// 0000 = min, 0120 = max.
    pub const ALC: u8 = 0x13;
    /// 0000 = 0 dB, 0130 = 15 dB, 0241 = 30 dB.
    pub const COMP: u8 = 0x14;
    /// 0000 = 0 V, 0013 = 10 V, 0241 = 16 V.
    pub const VD: u8 = 0x15;
    /// 0000 = 0 A, 0097 = 10, 0146 = 15, 0241 = 25.
    pub const ID: u8 = 0x16;
}

/// One sub-command of `16`, the switches. Manual 20-5 and 20-6.
pub mod function {
    /// 00 off, 01 on (144/430 MHz) or preamp 1 (HF/50 MHz), 02 preamp 2.
    pub const PREAMP: u8 = 0x02;
    /// 01 fast, 02 mid, 03 slow.
    pub const AGC: u8 = 0x12;
    pub const NOISE_BLANKER: u8 = 0x22;
    pub const NOISE_REDUCTION: u8 = 0x40;
    pub const AUTO_NOTCH: u8 = 0x41;
    pub const REPEATER_TONE: u8 = 0x42;
    pub const TONE_SQUELCH: u8 = 0x43;
    pub const SPEECH_COMPRESSOR: u8 = 0x44;
    pub const MONITOR: u8 = 0x45;
    pub const VOX: u8 = 0x46;
    pub const BREAK_IN: u8 = 0x47;
    pub const MANUAL_NOTCH: u8 = 0x48;
    pub const DTCS: u8 = 0x4B;
    pub const VSC: u8 = 0x4C;
    pub const TWIN_PEAK_FILTER: u8 = 0x4F;
    pub const DIAL_LOCK: u8 = 0x50;
    /// 00 sharp, 01 soft.
    pub const DSP_FILTER_TYPE: u8 = 0x56;
    /// 00 wide, 01 mid, 02 narrow.
    pub const NOTCH_WIDTH: u8 = 0x57;
    /// 00 wide, 01 mid, 02 narrow.
    pub const SSB_TX_BANDWIDTH: u8 = 0x58;
    /// DV mode only. 00 off, 01 DSQL, 02 CSQL.
    pub const DIGITAL_SQUELCH: u8 = 0x5B;
}

/// The frequency's width on the wire: five BCD bytes. Manual 20-11.
pub const FREQUENCY_BYTES: usize = 5;

/// A level's width: two BCD bytes, 0000 to 0255.
pub const LEVEL_BYTES: usize = 2;

/// One definition, spelled out.
///
/// Ten arguments, which clippy dislikes and which is right here: a
/// `CommandDefinition` has ten fields and a builder cannot be `const`.
/// The alternative is writing the struct literal out at every entry, which
/// is what this replaced.
#[allow(clippy::too_many_arguments)]
const fn def(
    id: Ic7100CommandId,
    code: (u8, Option<u8>),
    name: &'static str,
    description: &'static str,
    query_forms: &'static [CommandForm],
    set_forms: &'static [CommandForm],
    action_forms: &'static [CommandForm],
    response_forms: &'static [CommandForm],
    readable: bool,
    writable: bool,
) -> CommandDefinition<Ic7100CommandId, CivFormat> {
    CommandDefinition {
        id,
        code,
        name,
        description,
        query_forms,
        set_forms,
        action_forms,
        response_forms,
        readable,
        writable,
    }
}

const NONE: &[CommandForm] = &[];
const ACTION: &[CommandForm] = &[CommandForm::fixed(CommandOperation::Action, 0)];
const QUERY: &[CommandForm] = &[CommandForm::fixed(CommandOperation::Query, 0)];
const FREQ_SET: &[CommandForm] = &[CommandForm::fixed(CommandOperation::Set, FREQUENCY_BYTES)];
const FREQ_RESPONSE: &[CommandForm] = &[CommandForm::fixed(
    CommandOperation::Response,
    FREQUENCY_BYTES,
)];
/// A mode frame is the mode byte plus an optional filter byte: the manual
/// notes the filter may be omitted on commands 01 and 06.
const MODE_SET: &[CommandForm] = &[CommandForm::variable(CommandOperation::Set, 1, 2)];
const MODE_RESPONSE: &[CommandForm] = &[CommandForm::variable(CommandOperation::Response, 1, 2)];
const SELECTOR: &[CommandForm] = &[CommandForm::fixed(CommandOperation::Set, 1)];
const LEVEL_SET: &[CommandForm] = &[CommandForm::fixed(CommandOperation::Set, LEVEL_BYTES)];
const LEVEL_RESPONSE: &[CommandForm] =
    &[CommandForm::fixed(CommandOperation::Response, LEVEL_BYTES)];

const DEFINITIONS: &[CommandDefinition<Ic7100CommandId, CivFormat>] = &[
    // 20-3. Transceive: the radio volunteers these when its dial moves.
    def(
        Ic7100CommandId::SendFrequency,
        (0x00, None),
        "send frequency",
        "The operating frequency, sent unprompted when it changes",
        NONE,
        FREQ_SET,
        NONE,
        FREQ_RESPONSE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::SendMode,
        (0x01, None),
        "send mode",
        "The operating mode, sent unprompted when it changes",
        NONE,
        MODE_SET,
        NONE,
        MODE_RESPONSE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::ReadFrequency,
        (0x03, None),
        "read frequency",
        "Read the operating frequency",
        QUERY,
        NONE,
        NONE,
        FREQ_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::ReadMode,
        (0x04, None),
        "read mode",
        "Read the operating mode and filter",
        QUERY,
        NONE,
        NONE,
        MODE_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::SetFrequency,
        (0x05, None),
        "set frequency",
        "Set the operating frequency",
        NONE,
        FREQ_SET,
        NONE,
        NONE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::SetMode,
        (0x06, None),
        "set mode",
        "Set the operating mode, and optionally the filter",
        NONE,
        MODE_SET,
        NONE,
        NONE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::SelectVfo,
        (0x07, None),
        "select VFO",
        "Select VFO mode; 00 VFO A, 01 VFO B, A0 equalise, B0 exchange",
        NONE,
        SELECTOR,
        ACTION,
        NONE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::SelectMemory,
        (0x08, None),
        "select memory",
        "Select memory mode, or a channel as two BCD bytes",
        NONE,
        &[CommandForm::variable(CommandOperation::Set, 1, 2)],
        ACTION,
        NONE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::MemoryWrite,
        (0x09, None),
        "memory write",
        "Write the current state into the selected memory channel",
        NONE,
        NONE,
        ACTION,
        NONE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::MemoryToVfo,
        (0x0A, None),
        "memory to VFO",
        "Copy the selected memory channel to the VFO",
        NONE,
        NONE,
        ACTION,
        NONE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::MemoryClear,
        (0x0B, None),
        "memory clear",
        "Clear the selected memory channel",
        NONE,
        NONE,
        ACTION,
        NONE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::Scan,
        (0x0E, None),
        "scan",
        "Start or stop a scan; 00 stop, 01 programmed/memory, 22 memory",
        NONE,
        SELECTOR,
        NONE,
        NONE,
        false,
        true,
    ),
    def(
        Ic7100CommandId::SplitDuplex,
        (0x0F, None),
        "split and duplex",
        "Read or set split and duplex; 00 off, 01 on, 11 DUP-, 12 DUP+",
        QUERY,
        SELECTOR,
        NONE,
        &[CommandForm::fixed(CommandOperation::Response, 1)],
        true,
        true,
    ),
    def(
        Ic7100CommandId::TuningStep,
        (0x10, None),
        "tuning step",
        "Read or set the tuning step, 00 to 12",
        QUERY,
        SELECTOR,
        NONE,
        &[CommandForm::fixed(CommandOperation::Response, 1)],
        true,
        true,
    ),
    def(
        Ic7100CommandId::Attenuator,
        (0x11, None),
        "attenuator",
        "Read or set the attenuator; 00 off, 12 for 12 dB",
        QUERY,
        SELECTOR,
        NONE,
        &[CommandForm::fixed(CommandOperation::Response, 1)],
        true,
        true,
    ),
    // 20-4 and 20-5: the level and meter groups. Written out one
    // sub-command per definition rather than addressed generically,
    // because a *reply* has to resolve to a definition — a console that
    // could send `15 02` and not parse what came back would have a
    // meter it could ask about and never read.
    // 20-5, command 15 01: the squelch, open or closed.
    def(
        Ic7100CommandId::SquelchStatus,
        (0x15, Some(meter::SQUELCH_STATUS)),
        "squelch status",
        "Read whether the squelch is open",
        QUERY,
        NONE,
        NONE,
        &[CommandForm::fixed(CommandOperation::Response, 1)],
        true,
        false,
    ),
    def(
        Ic7100CommandId::MeterS,
        (0x15, Some(meter::S)),
        "s meter",
        "Read the S-meter level",
        QUERY,
        NONE,
        NONE,
        LEVEL_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::MeterPo,
        (0x15, Some(meter::PO)),
        "po meter",
        "Read the PO meter level",
        QUERY,
        NONE,
        NONE,
        LEVEL_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::MeterSwr,
        (0x15, Some(meter::SWR)),
        "swr meter",
        "Read the SWR meter level",
        QUERY,
        NONE,
        NONE,
        LEVEL_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::MeterAlc,
        (0x15, Some(meter::ALC)),
        "alc meter",
        "Read the ALC meter level",
        QUERY,
        NONE,
        NONE,
        LEVEL_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::MeterComp,
        (0x15, Some(meter::COMP)),
        "comp meter",
        "Read the COMP meter level",
        QUERY,
        NONE,
        NONE,
        LEVEL_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::MeterVd,
        (0x15, Some(meter::VD)),
        "vd meter",
        "Read the Vd (supply voltage) meter level",
        QUERY,
        NONE,
        NONE,
        LEVEL_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::MeterId,
        (0x15, Some(meter::ID)),
        "id meter",
        "Read the Id (drain current) meter level",
        QUERY,
        NONE,
        NONE,
        LEVEL_RESPONSE,
        true,
        false,
    ),
    def(
        Ic7100CommandId::LevelAf,
        (0x14, Some(level::AF)),
        "the AF level",
        "Read or set the AF level, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    def(
        Ic7100CommandId::LevelRfGain,
        (0x14, Some(level::RF_GAIN)),
        "the RF gain",
        "Read or set the RF gain, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    def(
        Ic7100CommandId::LevelSquelch,
        (0x14, Some(level::SQUELCH)),
        "the squelch level",
        "Read or set the squelch level, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    def(
        Ic7100CommandId::LevelNoiseReduction,
        (0x14, Some(level::NR)),
        "the NR level",
        "Read or set the NR level, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    def(
        Ic7100CommandId::LevelRfPower,
        (0x14, Some(level::RF_POWER)),
        "the RF power level",
        "Read or set the RF power level, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    def(
        Ic7100CommandId::LevelMicGain,
        (0x14, Some(level::MIC_GAIN)),
        "the microphone gain",
        "Read or set the microphone gain, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    def(
        Ic7100CommandId::LevelCwPitch,
        (0x14, Some(level::CW_PITCH)),
        "the CW pitch",
        "Read or set the CW pitch, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    def(
        Ic7100CommandId::LevelKeySpeed,
        (0x14, Some(level::KEY_SPEED)),
        "the keyer speed",
        "Read or set the keyer speed, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    def(
        Ic7100CommandId::LevelNotch,
        (0x14, Some(level::NOTCH)),
        "the manual notch position",
        "Read or set the manual notch position, 0000 to 0255",
        QUERY,
        LEVEL_SET,
        NONE,
        LEVEL_RESPONSE,
        true,
        true,
    ),
    // 1C 00: the transmit/receive state. Readable and writable, and the
    // one command in this table that can put a signal on the air.
    def(
        Ic7100CommandId::Transmit,
        (0x1C, Some(0x00)),
        "transmit",
        "Read or set the transmit/receive state",
        QUERY,
        SELECTOR,
        NONE,
        &[CommandForm::fixed(CommandOperation::Response, 1)],
        true,
        true,
    ),
    def(
        Ic7100CommandId::ReadId,
        (0x19, Some(0x00)),
        "read ID",
        "Read the transceiver's own CI-V address",
        QUERY,
        NONE,
        NONE,
        &[CommandForm::fixed(CommandOperation::Response, 1)],
        true,
        false,
    ),
];

/// Every command this radio's console uses.
///
/// The level (`14`), meter (`15`), function (`16`) and set-mode (`1A 05`)
/// groups are **not** enumerated here as one definition per sub-command.
/// Their sub-command byte selects which control, and the frame shape is
/// the same across each group — so they are addressed through
/// [`crate::command::level`], [`meter`], [`function`] and built at the
/// call site. Writing out two hundred near-identical definitions is how a
/// table drifts from the radio it describes.
pub const IC7100_COMMAND_TABLE: CommandTable<Ic7100CommandId, CivFormat> =
    CommandTable::new(DEFINITIONS);

#[cfg(test)]
mod tests {
    use super::*;
    use cat_framework::wire_format::CatWireFormat;

    fn civ() -> CivFormat {
        CivFormat::default()
    }

    #[test]
    fn every_command_byte_is_the_manuals() {
        // CI-V has no names, only numbers: a wrong byte here does not
        // fail, it runs a different real command. So each is checked
        // against 20-3 rather than trusted.
        for (id, code) in [
            (Ic7100CommandId::SendFrequency, (0x00, None)),
            (Ic7100CommandId::SendMode, (0x01, None)),
            (Ic7100CommandId::ReadFrequency, (0x03, None)),
            (Ic7100CommandId::ReadMode, (0x04, None)),
            (Ic7100CommandId::SetFrequency, (0x05, None)),
            (Ic7100CommandId::SetMode, (0x06, None)),
            (Ic7100CommandId::SelectVfo, (0x07, None)),
            (Ic7100CommandId::SelectMemory, (0x08, None)),
            (Ic7100CommandId::MemoryWrite, (0x09, None)),
            (Ic7100CommandId::MemoryToVfo, (0x0A, None)),
            (Ic7100CommandId::MemoryClear, (0x0B, None)),
            (Ic7100CommandId::Scan, (0x0E, None)),
            (Ic7100CommandId::SplitDuplex, (0x0F, None)),
            (Ic7100CommandId::TuningStep, (0x10, None)),
            (Ic7100CommandId::Attenuator, (0x11, None)),
            (Ic7100CommandId::ReadId, (0x19, Some(0x00))),
        ] {
            let d = IC7100_COMMAND_TABLE
                .definitions()
                .iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("{id:?} missing"));
            assert_eq!(d.code, code, "{id:?}");
        }
    }

    #[test]
    fn no_two_commands_share_a_code() {
        // Two definitions with the same code make the second unreachable,
        // and which one wins is lookup order rather than a decision.
        let mut seen = std::collections::HashSet::new();
        for d in IC7100_COMMAND_TABLE.definitions() {
            assert!(seen.insert(d.code), "{:?} duplicated by {:?}", d.code, d.id);
        }
    }

    #[test]
    fn a_frequency_read_round_trips_through_the_wire_format() {
        // The whole path: encode a request, take the radio's answer back
        // apart, and land on the right definition with the right payload.
        let f = civ();
        let request = f.encode_request((0x03, None), &[]);
        assert_eq!(request, vec![0xFE, 0xFE, 0x88, 0xE0, 0x03, 0xFD]);

        let reply = vec![
            0xFE, 0xFE, 0xE0, 0x88, 0x03, 0x00, 0x40, 0x07, 0x14, 0x00, 0xFD,
        ];
        let (definition, payload) = f
            .find_command(&IC7100_COMMAND_TABLE, &reply)
            .expect("a frequency reply");
        assert_eq!(definition.id, Ic7100CommandId::ReadFrequency);
        assert_eq!(
            cat_framework::civ::decode_bcd(payload),
            Some(14_074_000),
            "payload {payload:02X?}"
        );
    }

    #[test]
    fn a_sub_commanded_read_is_not_shadowed_by_a_bare_one() {
        // `19 00` is "read the transceiver ID". If lookup matched the bare
        // command first, every sub-commanded read would resolve to the
        // wrong definition -- and CI-V has enough of them that this is the
        // failure mode worth guarding.
        let f = civ();
        let reply = vec![0xFE, 0xFE, 0xE0, 0x88, 0x19, 0x00, 0x88, 0xFD];
        let (definition, payload) = f.find_command(&IC7100_COMMAND_TABLE, &reply).unwrap();
        assert_eq!(definition.id, Ic7100CommandId::ReadId);
        assert_eq!(payload, &[0x88]);
    }

    #[test]
    fn an_unknown_command_is_named_rather_than_guessed() {
        let f = civ();
        let reply = vec![0xFE, 0xFE, 0xE0, 0x88, 0x7E, 0x01, 0xFD];
        match f.find_command(&IC7100_COMMAND_TABLE, &reply) {
            Err(cat_framework::ParseError::UnknownCommand(what)) => {
                assert!(what.contains("7E"), "{what}");
            }
            other => panic!("expected an unknown command, got {other:?}"),
        }
    }

    #[test]
    fn a_mode_frame_may_omit_its_filter_byte() {
        // 20-11: "Filter setting can be skipped with command 01 and 06."
        // A form that demanded both bytes would reject the radio's own
        // shorter frames.
        let d = IC7100_COMMAND_TABLE
            .definitions()
            .iter()
            .find(|d| d.id == Ic7100CommandId::SetMode)
            .unwrap();
        assert!(d.supports(CommandOperation::Set, 1));
        assert!(d.supports(CommandOperation::Set, 2));
        assert!(!d.supports(CommandOperation::Set, 3));
    }

    #[test]
    fn the_meter_sub_commands_are_the_manuals() {
        // 20-5. These are constants rather than definitions, so nothing
        // else checks them.
        assert_eq!(meter::S, 0x02);
        assert_eq!(meter::PO, 0x11);
        assert_eq!(meter::SWR, 0x12);
        assert_eq!(meter::ALC, 0x13);
        assert_eq!(meter::COMP, 0x14);
        assert_eq!(meter::VD, 0x15);
        assert_eq!(meter::ID, 0x16);
    }

    #[test]
    fn every_meter_this_radio_declares_has_a_sub_command() {
        // The capability set and the command table have to agree, or a
        // console offers a meter nothing can read.
        use cat_framework::capabilities::MeterKind;
        for descriptor in crate::capabilities::IC7100.meters.meters {
            let (id, sub) = match descriptor.kind {
                MeterKind::S => (Ic7100CommandId::MeterS, meter::S),
                MeterKind::Po => (Ic7100CommandId::MeterPo, meter::PO),
                MeterKind::Swr => (Ic7100CommandId::MeterSwr, meter::SWR),
                MeterKind::Alc => (Ic7100CommandId::MeterAlc, meter::ALC),
                MeterKind::Comp => (Ic7100CommandId::MeterComp, meter::COMP),
                MeterKind::Vdd => (Ic7100CommandId::MeterVd, meter::VD),
                MeterKind::Id => (Ic7100CommandId::MeterId, meter::ID),
                // `MeterKind` is `#[non_exhaustive]`: a kind added
                // upstream that this radio does not declare cannot reach
                // here, because the loop walks *this radio's* meters.
                other => panic!("{other:?} is declared but has no CI-V sub-command"),
            };
            // Not merely that a constant exists: that the table has a
            // definition for it, so the radio's reply can be parsed. A
            // meter a console can ask about and never read is worse than
            // one it does not offer.
            let d = IC7100_COMMAND_TABLE
                .definitions()
                .iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("{:?} has no command definition", descriptor.kind));
            assert_eq!(d.code, (0x15, Some(sub)), "{:?}", descriptor.kind);
            assert!(d.readable, "{:?} is not readable", descriptor.kind);
        }
    }
}
