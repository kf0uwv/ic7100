# 1. Third radio on the shared CAT framework, via the new `CatWireFormat`/`CivFormat` seam

Date: 2026-08-09

## Status

**Superseded in part — see the 2026-09-04 amendment at the end.** All
three preconditions are now cleared and implementation has begun.

The original text follows, because what it demanded is exactly what was
done and the reasoning still governs.

**Blocked.** Three preconditions, none yet cleared:

1. `radio-cat-rs` (https://github.com/kf0uwv/radio-cat-rs) has generalized
   `cat-framework` over a `CatWireFormat` type parameter specifically so a
   binary/addressed protocol like Icom CI-V can be added without forking the
   engine — see that repository's
   [ADR 0009](https://github.com/kf0uwv/radio-cat-rs/blob/main/docs/adr/0009-civ-engine-for-binary-addressed-protocols.md).
   That work is real, committed, and verified (built and tested against both
   `ts570d` and `ft991a` with zero source changes to either), but as of this
   writing lives on an **unmerged branch**
   (`feat/civ-wire-format-adr-0009`), not `main`, and has **no tagged
   release**. `AsciiLineFormat` (the Kenwood/Yaesu shape) is the only
   `CatWireFormat` implementation that exists. **`CivFormat` — the actual
   CI-V binary-frame implementation this repo needs — has not been written
   at all.** This repo cannot depend on something that doesn't exist yet.
2. The official Icom IC-7100 CI-V Reference Guide has **not** been added to
   this repository (expected path: `docs/manuals/`). Preliminary web
   research on IC-7100 CI-V framing, addressing, and hardware ports exists
   (see the architecture discussion this ADR follows from), but that
   research explicitly flagged several facts as inferred/unverified rather
   than manual-cited (the USB bridge chip identity, whether USB-CI-V and the
   rear `[REMOTE]` jack can run concurrently, the exact firmware-E4 antenna
   CI-V opcode, and the IC-7100's own echo behavior). **It is not a
   substitute for the primary source** and must not be used as one when
   implementation actually starts — see `.claude/agents/icom.md` (once
   written) for the same read-the-manual-before-every-command discipline
   `ft991a`'s `yaesu.md` and `ts570d`'s `kenwood.md` already enforce.
3. An explicit go-ahead to begin `radio/` implementation itself has not been
   given. The architecture decisions this ADR records (generalize
   `radio-cat-rs` rather than fork it; defer the ACC-socket hardware adapter
   since CI-V already exposes S-meter/ALC/SWR/PO/Vd/Id/squelch/PTT without
   one) were made and are binding, but that is a decision about *shape*, not
   a dispatch order to start writing `Ic7100CommandId`/`IC7100_COMMAND_TABLE`
   entries.

This ADR records the target design now, before any of the three preconditions
clear, for the same reason `ft991a`'s own ADR 0001 did the same for *its*
preconditions: so that once `radio-cat-rs` publishes a tagged `CivFormat` and
the manual is in hand, this repo's implementation is a straightforward build
against a known, already-agreed shape — not a redesign done under pressure.

## Context

This repository is the **third** radio in the `radio-cat-rs` family, after
`ts570d` (Kenwood TS-570D) and `ft991a` (Yaesu FT-991A) — and the **first**
whose CAT protocol is not Kenwood/Yaesu-style ASCII line CAT at all. Icom's
CI-V protocol is architecturally different in every dimension that matters to
the shared engine:

| | `ts570d`/`ft991a` (ASCII CAT) | `ic7100` (CI-V) |
|---|---|---|
| Frame | `"<2-char code><params>;"` | `FE FE <to> <from> <cmd> [subcmd] [data] FD` |
| Command identity | 2-char ASCII string | 1-byte `cmd`, optional 1-byte `subcmd` |
| Data encoding | ASCII digits | binary BCD |
| Addressing | none — point-to-point | every frame carries controller/radio addresses; multi-drop bus; the radio's own address is a user-configurable runtime setting, not a protocol constant |

Auditing `cat-framework`'s actual code (not just its "generic CAT engine"
framing) found it was generic only across ASCII-line protocols — four spots
(`CommandTable::parse`'s `;`-scan/2-byte split, `CatClient`'s `format!("{code}
{params};")`, `SerialCatSession`'s read-until-`;` loop, `Broker::dispatch`'s
UTF-8 requirement) were hard-coded to that shape. `radio-cat-rs` ADR 0009
generalizes all four over a new `CatWireFormat` trait, with `AsciiLineFormat`
as a pure, behavior-preserving extraction of the existing logic and a future
`CivFormat` as the CI-V-native implementation — deliberately **one engine**
generic over the wire format, not a parallel framework per protocol family
(an earlier draft of that ADR proposed exactly that parallel-crate shape and
was rejected). This repo is the reason that generalization exists and the
first, and so far only, planned consumer of `CivFormat`.

## Decision

### Depend on the shared library, not on `ts570d`/`ft991a`, and not on a duplicated local engine

This repository will depend on the shared-library crates published by
`radio-cat-rs` — `cat-framework` (generic command table, parser, dispatch
lifecycle, response builder, `CatCommandCatalog`/`CatRadio` traits, now
generic over `CatWireFormat`), `cat-client`, `cat-transport-core`,
`cat-transport-serial`, and, for headless server/rigctl mode, `cat-server`/
`cat-rigctl` — as external dependencies, pinned to a released tag once
`radio-cat-rs` ADR 0009's branch merges and a new tag is cut (mirroring
`ts570d`'s and `ft991a`'s own `tag = "v0.1.0"` pins, not a floating
`branch = "main"`).

This repo will **not**:
- depend on `ts570d` or `ft991a` directly — they are siblings, not
  dependencies;
- vendor or duplicate a local copy of the generic CAT engine, or of the
  CI-V framing logic `CivFormat` is meant to own — no local
  `civ-framework`-equivalent crate is created here even temporarily to
  unblock early work (see Consequences for what to do instead).

### Shape of a third radio

Per `ts570d` ADR 0004's "Adding a second radio" section and `ft991a` ADR
0001's own application of it, this repo reuses the shared engine unchanged
and provides only its own:

- `Ic7100CommandId` enum;
- a static `IC7100_COMMAND_TABLE: cat_framework::CommandTable<Ic7100CommandId, CivFormat>`
  — explicit `CivFormat`, not the `AsciiLineFormat` default every existing
  radio's table relies on implicitly;
- a state machine (`Ic7100State`, transitions, state-dependent validation);
- `Ic7100Event` / `Ic7100Error` types;
- a `CatRadio<CivFormat>` implementation (`Ic7100Radio`) supplying command
  definitions and command semantics — the shared engine still handles
  framing, lookup, structural validation, and response formatting; this
  crate still only decides what each command *means*.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ic7100CommandId { Frequency /* ... */ }

struct Ic7100Radio { /* state machine fields */ }

impl cat_framework::CatCommandCatalog<CivFormat> for Ic7100Radio {
    type CommandId = Ic7100CommandId;
    fn command_table(&self) -> &'static cat_framework::CommandTable<Self::CommandId, CivFormat> {
        &IC7100_COMMAND_TABLE
    }
}
```

### The IC-7100 CI-V command set is not `ts570d`'s or `ft991a`'s — and isn't just "the other radios with a different wire format" either

Icom is a third manufacturer; nothing in `TS570D_COMMAND_TABLE` or
`FT991A_COMMAND_TABLE` transfers by assumption, structurally or by value —
the same standing rule `ft991a` ADR 0001 stated for `ts570d`, restated here
for a second sibling. But CI-V is a deeper divergence than "same shape,
different codes": command *identification* itself is nested (`cmd`, then an
optional `subcmd` byte disambiguating within it — e.g. `15 02` for S-meter,
`15 15` for supply voltage, both under `cmd 0x15`), parameters are BCD not
ASCII, and the protocol is address-framed on a shared bus rather than
point-to-point. `Ic7100CommandId`/`IC7100_COMMAND_TABLE`/`CivFormat` must all
be derived from the official Icom CI-V Reference Guide, command by command,
sub-command by sub-command — the same discipline `ts570d`'s `kenwood` agent
and `ft991a`'s `yaesu` agent applied to their own manuals. See Status above:
the manual is not yet present in this repository.

### Depend on the `CatSession` boundary from day one

Per `ts570d` ADR 0005 and `ft991a` ADR 0001's application of it, this repo's
controller client and UI must be transport-independent from the start:

- an `Ic7100<S: CatSession>` controller client, analogous to `ts570d`'s
  `Ts570d<S: CatSession>` and `ft991a`'s `Ft991a<S: CatSession>`;
- the UI depends on this repo's `Radio` trait and IC-7100 domain types only,
  never on a transport crate directly;
- only this repo's `src/main.rs` names a concrete `CatSession`
  implementation, choosing `SerialCatSession<SerialPort, CivFormat>` (or a
  TCP/UDP equivalent) explicitly — unlike `AsciiLineFormat`-based radios,
  which get `AsciiLineFormat` for free via `CatWireFormat`'s default type
  parameter, this repo's wiring layer must construct and pass a `CivFormat`
  instance (radio bus address, controller address) explicitly at every
  session/client/framework construction site, per `radio-cat-rs` ADR 0009's
  "format is a constructed value, not a bare type marker" design (CI-V's bus
  address is genuinely runtime configuration, unlike `AsciiLineFormat`,
  which carries none).

### PTT/keying: likely CI-V-command-driven, not RTS/DTR — to be confirmed against the manual, not assumed

`ft991a` ADR 0002 implemented PTT/CW keying over RTS (the FT-991A's `EX`
menu item 060 watches the CAT connector's RTS line directly; there is no
CAT-command PTT on that radio at all beyond `TX0;`/`TX1;`'s own limited
form). Preliminary research on the IC-7100 found the opposite default:
Hamlib's `ic7100.c` backend configures `ptt_type = RIG_PTT_RIG` — CI-V
command `0x1C 0x00` sets/reads PTT directly, no hardware line needed. If
that holds up against the actual manual, this repo likely does **not** need
a `CwKeying`/`ModemControlLines`-dependent trait the way `ft991a` did — but
this is exactly the kind of "near-universal concept, check the manual before
assuming another radio's mechanism" call `ft991a` ADR 0002 itself warns
against inverting, so it is **not** decided here. The `icom` agent (once
written) must confirm PTT's actual CI-V command against the manual before
any `transmit`/`receive` implementation lands, and only add a
`CwKeying`-equivalent trait if the manual documents a real hardware-line
keying mode analogous to `ft991a`'s menu item 060 (Icom radios of this era
sometimes expose one for CW break-in/RTTY FSK on the ACC socket's `FSKK`
pin, per the hardware research this ADR follows from — a distinct, ACC-pin,
non-CI-V mechanism, not a CAT-connector RTS line).

### Advanced hardware control mode: deferred, not part of this build

The IC-7100's ACC socket, DATA1/DATA2 jacks, and rear `[REMOTE]` CI-V jack
were surveyed as part of this repo's initial scoping (see the hardware
research the architecture discussion preceding this ADR produced). Finding:
S-meter, ALC, SWR, RF power, supply voltage/current, squelch status, and PTT
are all already exposed as CI-V read/write commands (`15 xx`, `1C 00`) — no
hardware tap is needed for any of them. The one thing that genuinely requires
one (analog band-voltage output on ACC pin 5, for automated antenna/amp band
switching) is disabled by default, requires an internal solder-bridge
modification to enable, and is likely superseded by a CI-V-based
"CI-V Output (for ANT)" path Icom added in firmware E4. Per an explicit
decision made when this project was scoped: **no external hardware adapter
is designed or built as part of this repository.** If a genuine need for one
surfaces later (confirmed not coverable via CI-V), it is a distinct, later
ADR — not speculative work done ahead of that need.

## Consequences

- `CivFormat`'s existence and tagged availability in `radio-cat-rs`, and the
  IC-7100 CI-V Reference Guide's presence in this repository, are both
  prerequisites outside this repo's control; this repo cannot get ahead of
  them by design (no vendored/duplicated CI-V framing implementation, no
  command table guessed from secondary sources).
- If implementation pressure arrives before `CivFormat` is ready, the correct
  response is to finish and tag it in `radio-cat-rs` (or explicitly re-scope
  this ADR), not to build a local CI-V engine substitute here — doing so
  would recreate exactly the coupling `radio-cat-rs` ADR 0009 was written to
  avoid, and would need to be un-done later.
- Because this repo depends on the shared library rather than on `ts570d` or
  `ft991a`, all three radio repos remain independent siblings: neither can
  break another, and a bug fix or feature in the shared engine (including
  `CivFormat` itself, once it exists) benefits all three once each bumps its
  dependency.
- The IC-7100 command table, state machine, and domain types are entirely
  new work requiring the Icom manual — none of `ts570d`'s or `ft991a`'s
  radio-specific code is reusable here beyond serving as a structural
  example of how a `CatRadio` implementation is organized, and even that
  example needs adapting for `CivFormat`'s nested `cmd`/`subcmd` identity and
  binary parameters, which neither sibling's code models at all.
- This ADR does not select a crate layout or workspace structure beyond
  naming the expected pieces (`radio`, `ui`, `emulator`, `server`, `src/main.rs`
  wiring) and the file-by-file scaffold list already produced during this
  project's initial scoping (mirroring `ft991a`'s layout) — a future ADR (or
  the architect's task plan) settles the concrete `Cargo.toml`/workspace
  shape once `radio-cat-rs` publishes a tagged `CivFormat` to depend on.
- The emulator (test infrastructure, PTY-hosted, per `ft991a`'s/`ts570d`'s
  pattern) will need genuinely new framing logic of its own — CI-V has no
  `';'`-terminated boundary to split on; it needs a real `FE FE...FD`
  state-machine scanner, plus a decision on whether to simulate bus
  addressing/echo/CI-V-Transceive broadcast behavior or simplify it away for
  test purposes. Not decided here — an `emulator`-agent-level task once
  `CivFormat` exists to build against.

---

## Amendment, 2026-09-04 — unblocked, and started

All three preconditions cleared:

1. **`CatWireFormat` is on `main`**, not a branch — `cat-framework`'s
   `wire_format` module, with `AsciiLineFormat` as the Kenwood/Yaesu
   shape. **`CivFormat` is now written** (`cat_framework::civ`): the
   `FE FE <to> <from> Cn [Sc] … FD` frame, addressing, echo rejection,
   and little-endian packed BCD.
2. **The manual is in `docs/manuals/`.** `IC-7100_Full_Manual.pdf`, from
   Icom America. Note for anyone repeating the search: the IC-7100 has no
   standalone "CI-V Reference Guide" — the document by that name on
   Icom's site is the **IC-R15's**, a different radio. The IC-7100's CI-V
   documentation is section 20 of its full manual.
3. **Go-ahead given.**

### What is built

- `cat_framework::civ` — the wire format. Three things the ASCII shape
  never had: frames are addressed (four radios share a bus), the radio
  **echoes** the controller's own frame back on a single-wire bus, and a
  command's identity is one or two bytes with no syntactic rule to
  separate them. `Code` is `(u8, Option<u8>)` because the manual's table
  is: `03` is a whole command, `15` alone means nothing.
- `radio::mode` — ten modes including **DV**, and the 0x08→0x17 gap the
  manual leaves. WFM is receive-only and says so.
- `radio::capabilities` — **seven meters**, each with the manual's own
  calibration, and an S-unit scale derived from its three points rather
  than transcribed.
- `radio::command` — the CI-V table, every code cited to a page.
- `radio::console_layout` — this radio's own console and palette.

- `radio::state` — what an IC-7100 is doing, including duplex, which
  neither other radio in this fleet has and which matters on the repeater
  bands this one reaches.
- `radio::ic7100_radio` — the `CatRadio` state machine. A read is answered
  with the command it was asked (CI-V has no response codes), a write with
  a bare `FB`, a refusal with `FA`.

### The byte-order trap

**Frequency is little-endian BCD and everything else is big-endian.**

That reads like a mistake and is not. The manual gives frequency its own
digit diagram at 20-11, annotating each byte — "1 Hz digit", "10 Hz
digit" — precisely because it reverses the order the rest of the document
uses. Every other multi-byte value is printed as a plain decimal (`0000 to
0255`, `0001–0099`) and goes on the wire in the order it is printed.

The first implementation sent levels in the frequency's order. Nothing
failed: the radio accepts the frame and sets something else, and an AF
level of 96 arrives as 150. It was caught by `examples/wire`, which prints
what the radio actually says so it can be read against the manual — a
question assertions cannot ask, because an assertion only checks that the
bytes are what the *test* expected.

`cat_framework::civ` has both codecs and says why on each.

- `radio::ic7100` — the typed client. Takes replies apart in one place,
  because CI-V answers a read with the command it was asked and twenty
  call sites each slicing an envelope by hand is twenty chances to be off
  by a sub-command byte.
- `emulator` — a virtual IC-7100 on a PTY, its meters computed from a
  synthetic band through *this* radio's S-unit calibration.
- `ic7100` — the wiring layer, and `--scan`, which a bus needs and a
  point-to-point protocol never does.

### Two things the framework had to grow

Both were gaps that only a binary protocol could expose:

- **`CatClient` returned `String`.** `from_utf8_lossy` is exactly right
  for an ASCII protocol and destroys a CI-V frame — `0x88` is not valid
  UTF-8 and arrives as U+FFFD, so a radio's address and half its BCD data
  would come back as replacement characters. There is a byte path now.
- **`SerialCatSession` read until `;`.** Hardcoded, so it waited forever
  for a semicolon a CI-V frame never contains; the symptom was a program
  that printed nothing and did not exit. It is generic over
  `FrameScanner` now, defaulted to `AsciiLineFormat` so every existing
  caller is unchanged. That trait had existed since ADR 0009 and nothing
  had used it.

### Verified end to end

App → CI-V session → PTY → emulator → state machine and back: the dial
reads 14.100.000 MHz, all seven meters report, `--scan` finds the
emulator at 88 and nothing else, and the S-meter tracks the band — S7 on
a signal at 13.6345 MHz, S1 five kilohertz away, S9 on a CW carrier at
25.597 MHz.

- `server` — rigctl for WSJT-X and the console protocol, both at once.
- `ui` — the terminal console, network-only.
- `gui` — a window around the shared GPU console.

### Three more gaps only a binary protocol could expose

Each was a place where something in the shared stack had quietly assumed
ASCII, and each was found by running the thing rather than by reading it:

- **The broker stringified every response.** `DispatchOutcome::Response`
  held a `String`, so a CI-V frame arrived at rigctl as replacement
  characters and every read answered `RPRT -1`. It carries bytes now, with
  `text()` for the protocols that have text.
- **`cat-rigctl` and `cat-server::build` were pinned to `AsciiLineFormat`.**
  Generic now, defaulted so no existing caller changed.
- **The runtime had no timer.** The broker times out a request the radio
  never answers, and `#[monoio::main]` without `timer_enabled` panics the
  moment it tries — with a message about the runtime, a long way from the
  cause. The other two apps had always set it.

### And one that was wrong for every radio

The terminal console's band and mode rows were **hardcoded to the
TS-570D's sets** — nine HF bands and six Kenwood modes — while the GUI
derived both from capabilities. An IC-7100 offered only HF on a radio that
reaches 430 MHz looks broken; an FT-991A showing six of its fourteen modes
hides most of the radio. Both rows derive from the capability document
now, which is what the rest of the console had always done.

### One thing worth flagging

`rx_range` is a single contiguous range in the capability model, and this
radio's coverage is not: 30 kHz–200 MHz **and** 400–470 MHz. The
declaration carries the outer envelope and over-claims the 200–400 MHz
gap; `capabilities::covers` is the honest answer and is what a band bar or
a tune command should ask. Closing that properly means a capability model
that holds more than one range, which is every radio's business rather
than this one's.
