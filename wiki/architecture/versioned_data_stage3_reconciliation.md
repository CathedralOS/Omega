# Reconciliation: shipped `Versioned<T>` stage 3 vs ch21's Decided Model

Written 2026-06-14. Chapter 21 grew a new top section, **"The Decided Model"**
(single-step `Upgradable<Old, New, Context>` + an owned `replace ... with` plan),
which explicitly supersedes the earlier exploration "where they conflict." The
shipped `Versioned<T>` container (frozen decision 14, landed stage 3 @8867e878)
is part of that earlier exploration. This note records how the two relate so the
stage-4 implementer does not build the wrong thing. **No chapter or code
behavior is changed here** — ch21 is the authority and is being actively edited;
this is analysis + a pointer.

## What shipped (stage 3)

`Versioned<T>` is a compiler-synthesized container `{ era: u32, payload:
union-of-all-declared-era-shapes }`, built for any `data` type that declares
`version vN { ... }` blocks. Era numbering is linear (decision 10's wire-era
assignment); ZII = era 0 = the oldest shape. Version-match arms
(`Counter::v1(old) -> ...`) desugar to era compares + payload member reads.
Naming contract: `omega-core/src/versioning.rs`. There is no source-level
constructor — only boundary machinery can mint a container — so today the only
runtime path is a ZII era-0 value.

## How the Decided Model repositions it

The decided model splits the concern in two, and `Versioned<T>` lands on the
**wire** side, not the live-state side:

| Concern | Decided owner | Shipped `Versioned<T>` fits? |
|---|---|---|
| Persisted/external data, many versions stale, reader tolerance | **Wire data (ch20)** | YES — this is exactly an era-tagged multi-version blob + era match |
| Live in-memory state across a hot swap | **`Upgradable` + `replace` plan (ch21)** | NO — live state is single-step `prev -> current`, no union, no multi-era dispatch |

Concrete divergences from the decided model (so they are not mistaken for bugs):

1. **Identity.** Shipped uses a linear `version vN` tag + `era: u32`. The decided
   model makes a shape's identity the **content hash of its canonical layout**,
   recorded in a build lockfile; `vN` is provenance only and editing a shipped
   shape is a compile error (drifted hash). The linear era is a wire-decode
   discriminator, not a live-state identity.
2. **Multiplicity.** Shipped stores a union of **all** declared era shapes and
   can match **any** historical era — multi-version coexistence. The decided
   model says live state is **single-step** (`prev -> current` only); "the chain
   and coexistence sketches ... were solving wire data's multi-version problem
   and do not apply to live state."
3. **Upgrade surface.** Shipped has no upgrade trait/plan. The decided model's
   live-state mechanism is `trait Upgradable<Old, New, Context = Nothing>`
   (resolution by TYPE, not a magic name) discharged by an owned, phase-checked
   `replace NetDriver.prev with NetDriver` plan (quiesce -> capture -> upgrade ->
   install), gated on an upgrade capability. None of that is built yet.

## Recommendation for stage 4 (no action taken now)

- **Do NOT** extend `Versioned<T>` into the live-state hot-swap path. Live state
  wants `Upgradable<Old, New>` + the `replace` plan, and only `prev`/`current`
  (two shapes), not a union of all eras.
- **Re-home** `Versioned<T>` mentally as the **wire-decode era matcher**
  (chapter 20): a boundary decoder reads a versioned wire blob, tags its era, and
  the existing version-match arms select the shape. That is its real, useful job,
  and it is consistent with "coexistence is wire data's problem." The first
  boundary constructor (wire decode / storage read) is still the right trigger
  for the true union layout (the interim struct-not-union note in versioning.rs
  stands).
- When the live-state path is built, it is a **separate** feature from
  `Versioned<T>`: the `Upgradable` trait + `replace`-plan checker, not an
  extension of the era container.
- `versioning.rs`'s provenance comment ("frozen decision 14, chapter 21 Version
  Matching") should eventually point at the wire-data role instead. Deferred:
  ch21 is being actively edited and decision 14's text in TASKS.md has not yet
  been reconciled by the maintainer. Update the comment once ch21 settles.

## Status

Analysis only. No behavior, naming, or canary changed. `Versioned<T>` stage 3
remains correct for what it does (era-tagged container + version-match arms);
the takeaway is *where the stage-4 effort goes* — the `Upgradable`/`replace`
live-state mechanism is net-new and must not be conflated with the era
container.
