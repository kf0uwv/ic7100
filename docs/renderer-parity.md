# Renderer parity

Required by radio-cat-rs ADR 0013. Parity is on **capabilities**, not on
pixels: every capability this console offers should be reachable in both
renderers, and each place it is not gets a row here naming the ground.

The three grounds ADR 0013 allows are:

- **(a) fidelity** — the renderers can't represent the thing equally well
- **(b) gesture** — the interaction has no sensible counterpart
- **(c) in progress** — not built yet, with a tracking item

Development cost is explicitly **not** a ground.

A capability both renderers have, invoked differently because the media
differ, is **not** an exception and needs no ground — it belongs under
"Reached by different gestures" below.

## Capability parity: TUI vs GUI

Both renderers exist (`ui/`, `gui/`) and both are thin wiring around the
shared consoles in `cat-ui-ratatui` and `cat-ui-egui`. Neither knows this
radio's name. What makes either one an IC-7100 console is the `LayoutSpec`
and `Theme` in `radio/src/console_layout.rs`, published by the server in
the handshake, plus `capabilities::IC7100`, from which both derive their
bands, modes and the seven meters.

That is why this table is short: structural parity is not a discipline
either console has to keep. It follows from both being handed the same
document.

| capability | missing from | ground | tracking |
|---|---|---|---|
| click-to-tune on the spectrum, with the zoom-and-repaint animation | TUI | (b) | A pointer gesture aimed at a continuous surface. The TUI reaches the same *destination* by typing a frequency on the command line; what it cannot reproduce is the gesture, not the capability. |
| custom panels (`PanelKind::Custom`) | TUI | (c) | `cat-ui-egui` exposes a `Widgets` painter registry so a radio can supply a panel the shared vocabulary lacks. `cat-ui-ratatui` has no such registry and silently skips an unrecognised panel rather than naming it. This radio's layout uses only standard panels today, so nothing is currently lost — but the escape hatch is one-sided, and a terminal console should at minimum draw the panel's name. |

## Reached by different gestures

Not exceptions. The same capability, invoked the way each medium invokes
things.

| capability | TUI | GUI |
|---|---|---|
| retune | command line, band bar keys, tuning keys | click the readout digits, drag the band bar, click a signal |
| mode change | mode bar keys | click the mode bar |
| meter reading | seven rows in the meter rail, drawn as bars | the same seven, drawn as arcs |
| AF scope / FFT | braille-cell columns | continuous traces |

## Permanent notes for this radio

| item | note |
|---|---|
| S-unit labels | `IC7100_S_UNITS` is calibrated from the manual's three points (0 = S0, 120 = S9, 241 = S9+60dB). The label set stops at S9+30 while the radio reports usefully to S9+60. Both renderers show the same thing, so this is not a parity gap — it is a shared limit, recorded here so it is not "fixed" in one console alone. |
| the 200–400 MHz gap | `rx_range` over-claims across it; `capabilities::covers(hz)` is the truth. Both consoles must ask `covers()` rather than the range, or they will offer a band the receiver does not have. |
| DV mode | Hamlib has no name for it; rigctl reports `PKTFM`. Console-side both renderers show `DV`, which is what the radio calls it. |
