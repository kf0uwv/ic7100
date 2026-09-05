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

//! What an IC-7100's console looks like.
//!
//! Authored here, not derived, and deliberately unlike the other two.
//! Same components; different radio.
//!
//! # Why this radio gets this arrangement
//!
//! **Seven meters.** S, PO, SWR, ALC, COMP, Vd and Id — three more than a
//! TS-570D and two more than an FT-991A, and the last two are supply
//! voltage and drain current, which nothing else in this fleet reports at
//! all. They are what an operator watches on this radio, so the rail is
//! the widest and tallest of the three and it comes first.
//!
//! **No spectrum.** This radio has no band scope, unlike its stablemates
//! the IC-7300 and IC-7610. So there is no spectrum panel and the room
//! goes to the workspace, as on the FT-991A.
//!
//! **HF through UHF, with a hole.** 30 kHz to 200 MHz and 400 to 470 MHz.
//! The band bar has far more to show than an HF-only radio's, so it gets a
//! row of its own across the top rather than a corner of a rail.

use cat_layout::{Child, LayoutSpec, Node, PanelKind, Rgb, Size, Theme};

/// The meter rail's width.
///
/// Wider than either other radio's: seven meters, and two of them are
/// labelled in volts and amps rather than in bare counts, which needs the
/// room a bare number does not.
const RAIL_W: u16 = 26;

const LEVELS_W: u16 = 26;

/// The console this radio asks for.
pub fn layout() -> LayoutSpec {
    LayoutSpec::new(Node::rows(vec![
        Child::panel(Size::Fixed(5), PanelKind::Readout),
        // Its own row, because this radio reaches from 160 m to 70 cm and
        // a band bar tucked into a rail would wrap.
        Child::panel(Size::Fixed(3), PanelKind::BandBar),
        Child::panel(Size::Fixed(6), PanelKind::QuickBar),
        Child::new(
            Size::Min(8),
            Node::columns(vec![
                Child::new(
                    Size::Fixed(RAIL_W),
                    Node::rows(vec![
                        // Seven meters. The tallest rail of the three, and
                        // the reason this radio's console reads as an
                        // instrument panel rather than a readout.
                        Child::panel(Size::Min(9), PanelKind::MeterRail),
                        Child::panel(Size::Fixed(5), PanelKind::AfScope),
                        Child::panel(Size::Fixed(5), PanelKind::AfFft),
                    ]),
                ),
                Child::panel(Size::Min(20), PanelKind::Workspace),
                Child::new(
                    Size::Fixed(LEVELS_W),
                    Node::rows(vec![
                        Child::panel(Size::Min(6), PanelKind::LevelsRail),
                        // Ten modes including DV, which needs more than a
                        // single row.
                        Child::panel(Size::Fixed(5), PanelKind::ModeBar),
                    ]),
                ),
            ]),
        ),
        Child::panel(Size::Fixed(1), PanelKind::Status),
        Child::panel(Size::Fixed(1), PanelKind::CommandLine),
    ]))
}

/// What an IC-7100 looks like.
///
/// A black body with a detachable head, and a monochrome dot-matrix LCD
/// with an amber backlight — Icom's own orange, which is also the colour
/// of the RIT and M-CH indicator LEDs above the dial (manual, front panel
/// description).
///
/// That puts it in the same family as the TS-570D's amber and away from
/// the FT-991A's blue TFT, so the two amber radios are separated by
/// **temperature** rather than hue: this one is a cooler, redder orange on
/// near-black, where the Kenwood is a warmer yellow-amber on charcoal.
/// Both are legible; neither is mistakable for the other at a glance.
pub fn theme() -> Theme {
    Theme {
        // The body: black, cool.
        background: Rgb::hex(0x0a0a0c),
        panel: Rgb::hex(0x15151a),
        // The LCD's amber, redder than the Kenwood's.
        ink: Rgb::hex(0xff9d3c),
        // A set value: brighter, as a backlit segment shows emphasis.
        accent: Rgb::hex(0xffe0b0),
        // The meters. Kept in the amber family so seven of them read as
        // one instrument rather than as a chart.
        signal: Rgb::hex(0xffc16e),
        warning: Rgb::hex(0xff4b3a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cat_layout::Area;

    fn area() -> Area {
        Area::new(0, 0, 130, 44)
    }

    #[test]
    fn this_radio_asks_for_no_spectrum_panel() {
        // It has no band scope. Placing a panel that could never fill
        // would draw an empty axis looking like a dead receiver.
        assert!(!layout().root.places(&PanelKind::Spectrum));
    }

    #[test]
    fn the_meter_rail_is_the_tallest_of_the_three_radios() {
        // Seven meters against four and five. The layout has to give them
        // room or two of them fall off the bottom.
        let rail = layout().find(area(), &PanelKind::MeterRail).unwrap();
        assert!(rail.height >= 9, "rail only {} tall", rail.height);
        assert!(rail.width >= 26, "rail only {} wide", rail.width);
    }

    #[test]
    fn the_band_bar_gets_its_own_row() {
        // 160 m to 70 cm is a lot of buttons. In a rail they would wrap,
        // and a control that moves as a window resizes is one an operator
        // has to look for twice.
        let band = layout().find(area(), &PanelKind::BandBar).unwrap();
        assert!(band.width > 100, "band bar only {} wide", band.width);
    }

    #[test]
    fn the_furniture_is_there() {
        let spec = layout();
        for f in [PanelKind::Status, PanelKind::CommandLine] {
            assert!(spec.root.places(&f), "{f:?} missing");
        }
    }

    #[test]
    fn it_still_draws_in_a_small_terminal() {
        let placed = layout().resolve(Area::new(0, 0, 80, 24));
        for must in [
            PanelKind::Readout,
            PanelKind::Workspace,
            PanelKind::CommandLine,
        ] {
            assert!(
                placed.iter().any(|p| p.kind == must),
                "{must:?} lost at 80x24"
            );
        }
    }

    #[test]
    fn its_look_is_not_the_other_amber_radios() {
        // The TS-570D is also amber, so these two are the pair most easily
        // confused. Different enough that an operator with both on the
        // bench can tell the consoles apart before reading a label.
        let ts570d_ink = Rgb::hex(0xffb347);
        assert_ne!(theme().ink, ts570d_ink);
        // Redder: less green in the ink.
        assert!(theme().ink.g < ts570d_ink.g);
    }
}
