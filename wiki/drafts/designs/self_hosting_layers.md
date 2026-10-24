# Self-hosting layer plan: D and C to alpha_bootstrap

Purpose: plan the layers of the Omega-written compiler C and the
Epsilon-written compiler D on the selected chain, one layer at a time, with
the semantics fixed by the specification and Rust used as a comparator.
This is a working plan, not a contract. The implementation snapshot and
measurements below retain their stated dates; they have not been remeasured.
Delete it when its layers exist and
their gates replace it, or when an owner ruling changes the chain.

Governing contracts: [bootstrap contract](../../../bootstrap/CONTRACT.md),
[minimization](../../../bootstrap/MINIMIZATION.md),
[compiler request](../../spec/build/compiler_request.md),
[Terminal Psi product](../../spec/terminal-psi/product.md),
[pipeline ownership](../../../omega-rust/pipeline.md). Execution status stays on
[TASKS.md](../../../TASKS.md) (`OMEGA-PRODUCT-COMPILER-SOURCE`) and
[TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md) (`OMEGA-D`, `OMEGA-C`).

## Where things stand (2026-10-20, `4bd2529e755`)

| Piece | State | Evidence |
| --- | --- | --- |
| Rust reference | Structurally complete, behaviorally open: 0 of 12 release rows closed when last measured (2026-09-24). Emitted binaries ran correctly; most failures were refusals to produce them. | [distance_to_completion](../measurements/distance_to_completion.md), [compiler_progress](../measurements/compiler_progress.md) — a month old; remeasure before relying on numbers |
| C, Psi half (`source/psi/`) | Lexer, parser, multi-package resolver, typing, checking (difference-bound zones, arrivals, contracts, crash routes, termination, domains), ownership. 2,223,369 source bytes. No Terminal Psi production. | [source/psi/README.md](../../../source/psi/README.md) |
| C, Omega half (`source/omega/`) | Request phases 0, 1, 2, 6 and the OCOUT encoder. 50,534 bytes. No realization, no backend. | [source/omega/README.md](../../../source/omega/README.md) |
| D (`bootstrap/5_omega/`) | Parser covering the current C roots, a scalar `u8` compile slice, an Alpha encoder with labels/fixups, request phases 0/1/2/6. 569,920 bytes. | [bootstrap/5_omega/README.md](../../../bootstrap/5_omega/README.md) |
| alpha_bootstrap backend in Rust | Not required and not present; Rust may report it as not implemented ([contract](../../../bootstrap/CONTRACT.md#selected-execution-chain)). | — |

Rust is a comparator for everything up to Terminal Psi. Past Terminal Psi,
C's only target on the self-host path is `alpha_bootstrap`, which Rust does
not emit. The comparison point there is execution: Rust's Terminal Psi
interpreter versus the Alpha tape, observed as `TerminalTraceV1`.

## The binding constraint is D, not C

D runs under three interpretation layers (Epsilon evaluator on Gamma
evaluator on Alpha). Recorded costs on macOS arm64:

- Checking D's own 509 KB closure: 3,305 s and 3.85 M pairs
  ([d-composition](../../../tests/epsilon/d-composition/README.md)).
- Compiling `machine answer() -> u8 { 42 }` with D: about 400 s and 1.81 M pairs
  on top of that.
- Twelve small parser invocations: 2,951 s total, most of it Epsilon
  checking D itself ([omega-parser](../../../tests/bootstrap/omega-parser/README.md)).
- The Epsilon edge accepts at most 4,194,252 bytes of source plus stdin
  ([evaluator entry](../../../bootstrap/4_epsilon/EVALUATOR_ENTRY.md)). D is
  570 KB today and must grow to a full compiler; C's Psi half alone is
  already 2.2 MB, before Terminal Psi production, the backend and the
  library closure it imports. The cap is private capacity and can be
  reprovisioned, but the time and pair cost per input byte is what decides.
- The selected pair arena is 3.42 billion cumulative pairs, never reclaimed.
- Only 34 nested non-tail Epsilon calls are available, so D stays
  worklist-driven.

No measurement yet gives D's cost per byte of real C source. Without it, the
layer design below cannot be checked for feasibility. That is why layer 0
is a measurement.

If the measurement shows `D -> omega0` is prohibitive after ordinary cleanup
and reprovisioning, that is the first
[owner-escalation trigger](../../../bootstrap/MINIMIZATION.md#owner-escalation),
and the decision goes to `OWNER_QUESTIONS.md`. The anticipated option is
already named: a "smaller bootstrap coverage profile" that reports
unsupported valid Omega as `Incomplete`
([compiler_request.md](../../spec/build/compiler_request.md), line 600). It is not
selected, and [CONTRACT.md](../../../bootstrap/CONTRACT.md) line 46 currently
requires D and C to implement the same complete language. Do not file that
question before the measurement exists; with no numbers it has nothing to
decide.

## Layer 0: measure D on real C source

Run D's existing parser on unmodified C files of increasing size (one
~20 KB file, one ~100 KB file, then a whole `source/psi/` subtree), through
the same chain as the omega-parser gate. Record wall time, pairs and
receipt-reconstruction time separately from Epsilon's check of D.

Output: seconds per source byte and pairs per source byte for parsing alone,
and the projection to C's full closure at today's size and at 2x. Parsing is
the cheapest layer; if parsing alone projects past the arena or past days of
run time, every later layer inherits that.

Done when the numbers are recorded in the omega-parser or d-composition
README with revision and host, and the projection is written here.

## The layers

One straight line, identical in C and D. Each layer has one input, one
output, a Rust comparison point and a gate. The first pass carries a
single tiny program through every layer; later passes widen each layer
row by row (the [fact loop](../../../AGENTS.md#the-sample-loop)), never
one layer to completion before the next exists.

| # | Layer | Input → output | Rust comparison | Notes |
| --- | --- | --- | --- | --- |
| 1 | Request | OCREQ bytes → package graph, source snapshot, admissions | Same OCREQ vectors, same OCOUT refusals (`tests/product/request/`) | Phases 3, 4, 5, 7, 8 are missing in both C and D. Phase 3 lineage encoding is still unspecified. |
| 2 | Lex | bytes → tokens | Rust stage 00 token stream | Done in C and D. |
| 3 | Parse | tokens → syntax tables | Rust stage 01 syntax | C covers the grammar minus the forms listed on `OMEGA-PRODUCT-COMPILER-SOURCE`; D covers C's own roots. |
| 4 | Resolve | syntax → symbol-resolved | Rust stage 02 | Handle-first symbols; module namespaces pending `MODULE-NAMESPACE-RESOLUTION`. |
| 5 | Type | resolved → typed | Rust stages 03, 03b | Generics by unification exist in C; trait requirement calls, conformance static arguments, `Id` for data identity remain. |
| 6 | Check and prove | typed → checked + obligations discharged | Rust stage 04 diagnostics | The largest layer: zones, ownership, loans, termination, domains, and the proof kernel (predicative sMLTT with Id, W, quotients). D must carry the same kernel. |
| 7 | Lower | checked → Terminal Psi (`PSITERM`) | Compare decoded modules and execution; compare bytes under matched identity, lowering and optimization profiles | The one portable boundary. Canonical encoding fixes the bytes of a particular module; independent frontends can produce different valid modules. |
| 8 | Realize for alpha_bootstrap | Terminal Psi + Build companion rows → Alpha operations | None in Rust; compare by execution | Provider selection, layout and calling plan for one target with no host ABI. |
| 9 | Emit | Alpha operations → raw tape, OCOUT | Execute the tape on the Alpha VM; compare `TerminalTraceV1` with Rust's Terminal Psi interpreter on the same PSITERM | 22 opcodes, 256 64-bit registers, flat 128 GiB memory, tape ≤ 16,777,212 bytes. D already has an encoder with labels and fixups to reuse. |

### Why layers 8 and 9 are small

Alpha has no host ABI or object format and has 256 registers, but its
[`call` and `ret`](../../../bootstrap/0_alpha/SEMANTICS.md#5-transition-rules)
use an explicit downward return-address stack. The compiler still needs a
calling plan for parameters and results, activation-relative memory homes,
preservation of values live across calls, and bounded, disjoint activation
storage separated from tape, data and the return stack. A first backend can
use fixed offsets within each activation, load into registers per operation,
and store back. That avoids a general register allocator and branch relaxation;
it does not remove activation layout or call preservation obligations.
It may produce a slow omega0. The D edge runs through the interpreter chain;
C compiling itself (`omega0 -> omega`) runs on Alpha directly. Register homes
are an optimization to add only if measured compilation time requires them.

### Why layer 7 is the checkpoint

Everything before Terminal Psi has a Rust counterpart and everything after
does not. Under matched identity allocation, lowering and selected Psi
optimization profiles, byte-identical PSITERM is a useful differential check.
The [encoding contract](../../spec/terminal-psi/encoding.md#canonical-form)
canonicalizes one module; it does not require independent frontends to choose
that same module. [Selected optimization](../../spec/terminal-psi/product.md#optimization-ordering)
may also change it. When modules differ, inspect the decoded difference and
compare their execution under the same observation profile. Compare D's
checkpoint by the same rules before investigating its Alpha realization.
Agreement is diagnostic evidence; it does not discharge the bootstrap
[refinement obligations](../../../bootstrap/CONTRACT.md#non-lockstep-refinement).

This requires D to produce PSITERM as an internal artifact even though its
product is a tape. That is not an extra stage; it is the same layer boundary
C has, and it is what lets D be compared at all.

## Order of work

1. Layer 0 measurement. Decide whether the plan is feasible as written.
2. Vertical slice in C: `machine answer() -> u8 { 42 }` through layers 1–9
   in `source/`, emitting an Alpha tape that exits 42. Compare its PSITERM
   with Rust's and its trace with Rust's interpreter.
3. The same slice in D, reusing D's existing encoder. D's existing scalar
   path gets replaced by the real layer 7→9 route rather than extended in
   parallel.
4. Widen row by row: each new construct goes through every layer in C, then
   D, with its reject twin. Choose constructs by what C's own source needs
   first, since `OMEGA-D`'s acceptance is D compiling C.
5. When D can compile C: `D -> omega0`, then `omega0 -> omega`, then
   `omega` recompiles C deterministically (`OMEGA-C`).

Rust completion continues independently. The comparison points above need
Rust to be correct on the constructs being compared, not to have closed its
release gates.

## Open design questions (engineering, not owner)

These are delegated engineering per
[OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md) and
[pipeline ownership](../../../omega-rust/pipeline.md); they are the
candidates to hand to a stronger model for alternatives.

- **D's data layout.** Epsilon has fixed arrays, `i32` indices, no heap and
  no recursive types. Every C table must have a D counterpart with a declared
  capacity. Which C representations transliterate directly, and which need
  a different shape for D's cost model (each array update is O(log E) pair
  allocations that are never reclaimed)?
- **The proof kernel in D.** Layer 6 includes a dependent type theory with
  W-types and quotients. The cost of checking proofs inside three
  interpreters is unmeasured. Measure it on the proofs C's own source
  actually contains, not on the full language.
- **Terminal Psi writer size.** About 126 operation tags and 37 module
  tables. C and D both need a writer; only the tags C's source uses are
  needed for the self-host edge, but the full set is needed for the
  same-language requirement.
- **Fixed homes versus register homes** on Alpha (layer 8), decided by
  measured `omega0 -> omega` time.
- **Library closure.** C imports parts of `source/library` (1.28 MB in total).
  The bytes D must read are C plus its imported library modules, not the
  whole library; measure that closure before projecting input size.

## Owner question kept ready, not filed

If layer 0 projects `D -> omega0` as prohibitive, the question for the
owner is whether to select the smaller bootstrap coverage profile already
anticipated in [compiler_request.md](../../spec/build/compiler_request.md)
(D compiles the Omega that C uses and reports the rest as `Incomplete`),
or to change the chain another way. It must cite the measurement, CONTRACT.md
line 46 (full language required), MINIMIZATION.md's escalation list, and
show the cleanup and reprovisioning already tried.
