# Icom IC-7100 Radio Control - Agent Guidelines

## Superpowers Coding Model (MANDATORY)
- Use planning-with-files skill for ALL implementation work
- Follow TDD, frequent commits, verification-before-completion
- Check for applicable skills BEFORE any action

## Planning-with-Files Requirement
- Each agent and subagent maintains planning files under `./planning/{agent}/`:
  `task_plan.md`, `findings.md`, `progress.md`
- An agent owns ONLY its own directory and never edits another's
- Planning files are created BEFORE any implementation work

## Read the manual first — this is not optional here

`docs/manuals/IC-7100_Full_Manual.pdf` (Icom America), with the CI-V section
extracted to `ci-v-section.txt` for grepping. Every command number, mode
byte and meter calibration point in this repo cites the page it came from.

**CI-V has no names, only numbers.** On the Kenwood and Yaesu radios a
malformed command is rejected as a bad string. Here, a wrong byte is a
*different valid command* — the radio does something real and reports
success. Tests do not protect you from this: an assertion checks the bytes
against what the test author expected, not against what Icom documented.

`cargo run -p radio --example wire` prints the actual frames this repo
produces, for reading against the manual. That is what caught the byte-order
trap below, after every test was already green.

## CI-V, and the three ways it differs from the ASCII radios

```
FE FE <to> <from> <Cn> [<Sc>] [data…] FD
```

1. **Frames are addressed.** Radio `0x88`, controller `0xE0`. Several
   radios share one bus, so a frame not addressed to us is not ours to
   parse. See `cat_framework::civ::is_for_us`.
2. **The radio echoes our own frame back** before answering. Failing to
   drop the echo reads as the radio replying instantly with our question.
   See `is_echo`.
3. **Byte order is not uniform.** *Frequency is little-endian BCD;
   everything else is big-endian.* There are two codecs in
   `cat_framework::civ` for exactly this reason — `encode_bcd`/`decode_bcd`
   (LE) and `encode_bcd_be`/`decode_bcd_be` (BE). Sending a level
   little-endian does not fail; 96 arrives as 150.

Anything that stringifies bytes (`from_utf8_lossy`) is correct for the
ASCII radios and destroys a CI-V frame. `CatClient` therefore offers
`query_bytes` / `query_with_bytes` / `set_bytes`; use those.

Framing reads until `FD`, not until `;`. `SerialCatSession` is generic over
`FrameScanner` for this — never hardcode a terminator.

## Core Technologies
- monoio: io_uring async runtime, **Linux only**, target-gated
- `#[monoio::main(timer_enabled = true)]` wherever a broker runs — without
  it the runtime panics the first time anything sets a timeout, with a
  message that points at the runtime rather than the cause
- ratatui + crossterm (TUI); egui/wgpu (GUI)
- **Tokio must NEVER be used in this project**

## Crate Dependency Model (MANDATORY)

The generic engine, transports, protocol and both consoles are external,
from the sibling repo `radio-cat-rs`. Do not vendor or fork them here.

```
radio     (cat-framework, cat-client, cat-transport-core — never a concrete transport)
  └── Ic7100CommandId + the single command table (command.rs)
  └── Ic7100Radio (CatRadio impl + emulator state machine) (ic7100_radio.rs)
  └── Ic7100<S: CatSession>, the typed client (ic7100.rs)
  └── Mode, capabilities, state (mode.rs, capabilities.rs, state.rs)
  └── console_layout.rs — THIS radio's LayoutSpec + Theme

ui        wiring around the shared cat-ui-ratatui console (network-only)
gui       wiring around the shared cat-ui-egui console
          — never depends on radio, cat-framework or cat-transport-*
            (radio is a dev-dependency for examples/render.rs only)
server    publishes capabilities, layout and theme; serves the native
          protocol AND rigctl at once
emulator  PTY + civ_loop framing; meter derived from the synthetic band
src/main.rs  the wiring layer — the ONLY place a transport is named
```

### Rules (violation is a blocking issue)
1. `radio` never imports a concrete transport crate.
2. There is exactly ONE command table.
3. `gui` never links `radio`, `cat-framework` or any `cat-transport-*`.
4. `src/main.rs` is the only place concrete types are wired together.
5. Unit tests use fakes of the relevant trait, never a real transport.
6. Engine or transport changes belong in `radio-cat-rs`, not here.

## The console is authored here, rendered there

Both consoles are shared with every other radio and neither knows this
radio's name. What makes the IC-7100 console an IC-7100 console is the
`LayoutSpec` and `Theme` in `radio/src/console_layout.rs`, which the server
publishes in the handshake. A console derives its bands, modes and meters
from the capability document — **never hardcode a band or mode list in a
renderer.** If a radio needs a panel the shared vocabulary lacks, use
`PanelKind::Custom(name)` and supply the painter (egui has a `Widgets`
registry for this; the ratatui console does not yet — see below).

## IC-7100 specifics worth knowing
- Ten modes; the byte gap 0x08 → 0x17 is real — `Dv = 0x17`. Do not compact it.
- WFM is receive-only (`can_transmit() == false`).
- Seven meters: S, Po, SWR, ALC, Comp, Vdd, Id.
- `IC7100_S_UNITS` is derived from three calibration points (0 = S0,
  120 = S9, 241 = S9+60dB) as **midpoint** thresholds. Unit boundaries put
  S9 in S8 — that mistake has already been made once.
- `covers(hz)` exists because the receiver has a real 200–400 MHz gap.

### Recorded limits (not bugs, but do not "fix" them blindly)
- `rx_range` over-claims across the 200–400 MHz gap; `covers()` is the truth.
- DV maps to Hamlib `PKTFM` — the closest thing rigctl has.

## Essential Commands
- Build / test: `cargo build --workspace` / `cargo test --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt`
- Emulator: `cargo run -p emulator` (prints the PTY path to use as `--port`)
- Server: `cargo run -p ic7100 -- server --port <pty> --console-port 4001 --rigctl-port 4532`
  (`--console-port` is the native console protocol, `--rigctl-port` the
  Hamlib bridge; also `--raw-tcp-port` / `--raw-udp-port`. Both listeners
  run at once — they are not alternatives.)
- TUI: `cargo run -p ic7100 -- --console 127.0.0.1:4001`
- GUI: `cargo run -p gui -- --server 127.0.0.1:4001`
- Bus scan: `cargo run -p ic7100 -- --port <pty> --scan`
- Wire check: `cargo run -p radio --example wire`
- Offscreen render: `cargo run -p gui --example render -- out.png`

## `rigctl_radio.rs` is a seam, not debt

`cat-rigctl` owns the rigctl protocol and is driven by `RadioCapabilities`.
What stays here is the mode-name mapping, because that is irreducibly
per-radio: this radio has `DV`, which Hamlib has no name for, and a `WFM`
it will not transmit in. Keep the file thin; anything capability-shaped
belongs in `cat-rigctl`.

## Known gaps
- `cat-ui-ratatui` has **no custom-widget registry**; a terminal console
  meeting a `PanelKind::Custom` skips the panel rather than naming it.
- Dependencies on `radio-cat-rs` are `path` deps and must become tagged git
  deps when that library cuts a release.

## Code Style
- Imports: std → external → local
- Errors: thiserror + `Result<T, E>`
- snake_case / PascalCase
