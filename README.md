# ic7100

Control an Icom IC-7100 over CI-V, on the shared CAT framework from
[`radio-cat-rs`](https://github.com/kf0uwv/radio-cat-rs).

The third radio in this fleet, after `ts570d` (Kenwood) and `ft991a`
(Yaesu) — and the first on a **binary, addressed** protocol rather than
an ASCII line one.

## Running it

```sh
# A virtual IC-7100 on a pseudo-terminal. Prints PTY_SLAVE=/dev/pts/N.
cargo run -p emulator

# Read what it is doing.
cargo run --bin ic7100 -- --port /dev/pts/N

# Move the dial first.
cargo run --bin ic7100 -- --port /dev/pts/N --tune 13634500

# Find radios on a bus whose addresses nobody wrote down.
cargo run --bin ic7100 -- --port /dev/pts/N --scan
```

Two diagnostics worth knowing about:

```sh
# What this radio actually says on the wire, for reading against the
# manual's frame diagram at 20-2.
cargo run -p radio --example wire

# Whether the emulator's S-meter agrees with the band it is receiving.
cargo run -p emulator --example agree
```

## What is here

| crate | what |
|---|---|
| `radio` | the CI-V command table, capabilities, state machine, and the typed client |
| `emulator` | a virtual IC-7100 on a PTY, with meters computed from a synthetic band |
| `ic7100` (root) | the wiring layer: the only place a transport is named |

The wire format itself — `FE FE <to> <from> Cn [Sc] … FD`, addressing,
echo rejection, BCD — lives in `radio-cat-rs`'s `cat_framework::civ`,
because it is Icom's and not this radio's.

## Three things CI-V does that the ASCII radios do not

- **Frames are addressed.** Four radios share a bus. `--scan` exists
  because of this and has no counterpart on the other two.
- **The radio echoes.** On a single-wire bus a controller's own frame
  comes back before the answer. A client that took the echo for the reply
  would display every value it had just asked for.
- **Byte order is not uniform.** Frequency is little-endian BCD;
  everything else is big-endian. See `cat_framework::civ`, which says why
  and has both codecs.

## The manual is the source

Every command, mode byte and meter calibration in `radio` cites the page
of `docs/manuals/IC-7100_Full_Manual.pdf` it came from. CI-V has no
names, only numbers: a wrong byte does not fail, it runs a *different real
command*. Check the manual before writing one — see `docs/adr/0001` and
the same discipline `ts570d`'s `kenwood.md` and `ft991a`'s `yaesu.md`
enforce.

## Sharing it

```sh
# Both listeners at once. They are not alternatives.
cargo run --bin ic7100 -- server --port /dev/pts/N \
    --rigctl-port 4532 --console-port 4533

# The terminal console, against that server.
cargo run --bin ic7100 -- --console 127.0.0.1:4533

# The GPU console.
cargo run -p gui
```

Both consoles are **shared code**: the arrangement and the palette come
from `radio::console_layout` over the protocol, and the panels are
`cat-ui-ratatui`'s and `cat-ui-egui`'s. This repo's `ui` crate is the
loop that feeds one; its `gui` crate is a window.

The terminal console is **network-only**, unlike the other two radios'.
Theirs grew a local serial path first and a network one later and carry
both; this one starts where that ended up.
