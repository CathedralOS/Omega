# Tasks

Compiler/runtime work surfaced by the `dungeon_crawler_cli` sample, the canary ladder, and the current language-design push.

## High Priority

- [ ] Termination proofs
  Add an opt-in termination proof surface that can be claimed at roots such as `Main::main`, then enforced transitively through the reachable call/state graph.
  Language direction:
  - `terminates`
  - nested progress clauses under `terminates`
  - `decreases value -> OrderOrMeasure`
  - builtin ranking views for naturals, bounded distances, and slice lengths
  - plain `decreases value` only when builtin/default ranking is unambiguous
  Prototype already landed:
  - bare `terminates` plus bare `decreases expr` parse/lower through syntax/resolved/typed trees
  - checked-tree validation rejects direct terminating recursive cycles with no `decreases`
  - direct countdown recursion proves
  - arithmetic bounded-distance recursion proves
  - slice-backed bounded-distance recursion proves
  - explicit `decreases value -> Nat::Descending` now parses/lowers/checks for the currently supported countdown and bounded-distance shapes
  - explicit `decreases entries -> Slice::Length` now proves for the first shrinking-subslice self-loop shape
  Next implementation steps:
  - broaden explicit `decreases value -> OrderOrMeasure` beyond `Nat::Descending`
  - keep bare `decreases value` only for unambiguous builtin cases
  - migrate current arithmetic-facing proof logic behind named builtin ranking views instead of exposing `limit - index` as primary UX
  - make canaries follow the new surface instead of the prototype spelling
  First builtin ranking views:
  - `Nat::Descending`
  - bounded distance to a limit/bound
  - `Slice::Length`
  Canary ladder:
  - terminating countdown loop with builtin descending natural order
  - terminating index-carrying loop with named bounded-distance order
  - terminating slice loop with `decreases items -> Slice::Length`
  - shrinking-slice loop with plain `decreases items`
  Then:
  - lexicographic rankings
  - named multiple orders for the same data type
  - custom ranking projections/orders for user-defined structs
  - cycle/SCC coverage beyond the current narrow direct recursion shapes
  Immediate blockers:
  - migrate the prototype away from bare arithmetic-facing `decreases expr`
  - runtime subslice descriptor semantics are still wrong for simple `tail.len` probes, so shrinking-slice proofs are ahead of runtime slice behavior
  - invalid subslice bounds like `view[9..]` are still accepted instead of requiring a proof-backed bounds check
  - plain `decreases items` still needs builtin/default ranking inference rather than only explicit `-> Slice::Length`

- [ ] Domain operators and proof-aware operator resolution
  The executable domain surface is now much healthier; the next step is turning the domain-operator idea into a real compiler feature rather than just documentation.
  Next target:
  - define the first concrete operator-resolution surface driven by proved domains
  - keep ambiguity rules strict and compile-time only

- [ ] Proof-checking depth beyond current domain coverage
  We have broad coverage now for domains flowing through calls, exits, mutation invalidation, indexing, and boolean implications.
  Next target:
  - deeper proof shapes that are not just more symmetry
  - quantified/sequence-style facts
  - termination-ranking proof facts
  - custom well-founded projections

- [ ] Persistent machine/state mutation confidence
  Make writes in one state reliably observable in later states and transitions, with regression coverage for room/event flags, counters, and re-entry behavior.
  Remaining target:
  - keep extending confidence on broader multi-edge/full-package flows
  - continue strengthening generator-style nested storage updates
  - keep pushing toward the remaining generic dungeon-sample blockers rather than only isolated micro-shapes

- [ ] Slices as a first-class proof/runtime feature
  The basic slice ladder is now in good shape; the next work should move from “individual seams compile/run” toward stronger semantic support.
  Next target:
  - runtime subslice/range descriptor semantics beyond the current compile/proof surface
  - proof-backed rejection of invalid subslice bounds
  - plain shrinking-slice ergonomics after `decreases value -> Slice::Length`
  - stronger proof vocabulary around slice windows and non-empty views
  - more complex alias/proof interactions over slice-backed structures

- [ ] Runtime text and `read_line` confidence
  Stabilize mutable runtime text/string handling and keep broadening real IO coverage.
  Next target:
  - multi-step text flows with richer state transitions
  - more host/runtime confidence around real console interaction paths

## Backend Quality

- [ ] Strengthen assigned-target allocation
  Evolve the current assigned-home model into a more mature register/stack allocation story with clearer register classes, spill behavior, and post-assignment cleanup.

- [ ] Reduce host/runtime special-case lowering
  Keep shrinking the bring-up-era special handling around stdin/stdout/process calls so host/runtime lowering feels like a real subsystem instead of a narrow happy path.

## Language Guide

- [ ] Continue guide reorganization past the front half
  The front-half sequencing is much cleaner now.
  Next target:
  - traits/modules/host-boundaries sequence
  - keeping advanced chapters from assuming concepts that have not been introduced yet
  - pulling speculative topics into clearer “working direction” sections when needed
