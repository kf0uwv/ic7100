# Architecture Decision Records

Decisions are recorded as [ADRs](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
(Michael Nygard format). Each file is one decision; numbers are stable and never reused.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-third-radio-civ-on-the-shared-cat-framework.md) | Third radio on the shared CAT framework, via the new `CatWireFormat`/`CivFormat` seam | Blocked |

## Repository status

**Scaffolding only — blocked on three preconditions, none yet cleared.** See
[ADR 0001](0001-third-radio-civ-on-the-shared-cat-framework.md)'s Status
section for the exact, current gating detail; summarized here:

1. `radio-cat-rs` generalized `cat-framework` over a `CatWireFormat` type
   parameter specifically to support this repo
   ([`radio-cat-rs` ADR 0009](https://github.com/kf0uwv/radio-cat-rs/blob/main/docs/adr/0009-civ-engine-for-binary-addressed-protocols.md)),
   verified against both sibling radios with zero source changes to either —
   but that work sits on an **unmerged branch**
   (`feat/civ-wire-format-adr-0009`), not tagged, and only adds the *seam*.
   **`CivFormat` — the actual Icom CI-V binary-frame implementation — does
   not exist yet.**
2. The official Icom IC-7100 CI-V Reference Guide has not been added to this
   repository. Preliminary web research on CI-V framing/addressing and the
   IC-7100's hardware ports exists but is explicitly not a substitute for
   the primary source (see ADR 0001's Status section for exactly what it
   left unverified).
3. No go-ahead yet to begin `radio/` implementation itself.

Also decided, and binding regardless of when the above clear: no external
hardware control adapter is in scope for this repository (CI-V already
exposes S-meter/ALC/SWR/PO/Vd/Id/squelch/PTT without one — see ADR 0001).

No code, crates, or workspace exist in this repository yet.
