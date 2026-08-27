# Delta rung

This directory canonically owns the Delta language corpus, the current
Delta-written self-hosting/reference compiler experiment, and lattice-built
Delta artifacts. Architectural ownership is independent of the language used
to produce an artifact. The final Delta program on the hosted path is
`omega-bootstrap`, which accepts `Ωself` and builds the full production Omega
compiler.

Delta is an independent compiler-host language, not an Omega subset. Its v1
contract is being discovered from the complete canonical Delta-compiler and
`omega-bootstrap` source closures plus explicit coherence, safety, robustness,
and maintainability arguments, and is tracked in
[`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

The corpus and disposable Rust producer are discovery inputs, not a feature
vote. A construct belongs to Delta v1 when the canonical compiler or bridge
demonstrates a concrete need, or an explicit language-coherence, robustness,
safety, or maintainability argument shows that retaining it reduces
whole-bootstrap cost; its lower-rung meaning must also close. Shared constructs
should retain Omega spelling and ordinary meaning where that is cheap. Source
outside Delta v1 rejects as Delta source; rejection of Omega outside `Ωself` is
`omega-bootstrap`'s separate responsibility. The working host surface is sealed
byte input, artifact output, diagnostic output, and process termination rather
than a general boundary-trait system.

Delta may present a general runtime-sized allocation interface to its programs
while implementing it with fixed, bump, typed/indexed-arena, or paged backing.
That is an ordinary compiler-host facility, not permission for an ambient host
heap: capacity, lifetime/reclamation, aliasing, and exhaustion behavior must be
part of the Delta contract and lower-rung meaning. The final choice remains
source- and cost-driven in the feature ledger.

- [`samples/`](samples/) contains the executable language corpus.
- [`FEATURE_LEDGER.md`](FEATURE_LEDGER.md) tracks provisional candidates and the
  evidence required to retain or remove them before the v1 freeze.
- [`DELTA_SOURCE_CLOSURE_SNAPSHOT_V1.md`](DELTA_SOURCE_CLOSURE_SNAPSHOT_V1.md)
  defines the path-independent closure format. The first snapshots bind the
  exact canonical compiler source image and one explicitly provisional
  three-root fixed-`u64` bridge action DAG. Filesystem locators and platform
  signing are diagnostic/staging data; source bytes, build edges, and normalized
  unsigned tool content are semantic commitments. These snapshots validate the
  manifest machinery but are not the final complete Delta-compiler or bridge
  closures.
- [`samples/lowermachine.alp`](samples/lowermachine.alp) is the Delta-written
  Delta-to-ARM64 compiler and self-host fixed point. Its native publisher
  batches `write_byte` output in a fixed 4 KiB buffer, flushes before ordered
  line output/input, explicit or implicit machine/state return, and traps, and
  retains direct regression teeth at the buffer boundary. A state that reaches
  its closing brace without a transition or explicit return returns zero; it
  never falls into the next lexical state. This is sealed host I/O, not a
  general allocator or runtime. Its fixed 21,528-cell typed backing admits at
  most 128 machine declarations, including the entry machine, 512 disjoint
  aggregate parameter rows—the complete product of that machine ceiling and
  D0's four-register value-parameter profile—and 512 disjoint field-metadata
  rows. The adjacent 129th machine or 513th field declaration, aggregate
  overflow, or over-wide signature exits with the established
  storage/array exhaustion status `3` before publishing output; exact-bound
  gates also resolve and call names beyond the former 64-row parameter
  partition through both native and self-built compilers. Its explicit byte
  arena currently reserves 512 KiB so the growing general bridge compiler fits
  without making source compaction part of the language contract; the adjacent
  512-KiB-plus-one input still fails closed before compilation or output. State
  declarations are joined to the exact phase-1 machine/state identity before
  their comment-aware balanced headers are consumed, so contextual identifiers
  such as `state`, `let`, `write_byte`, and `read_byte` cannot re-enter the
  statement or boundary-intrinsic dispatcher. A focused gate requires exact
  Rust/native/self assembly identity and execution across twelve such names.
- [`build/`](build/) contains the checked-in bootstrap compiler artifacts.
- [`rust/`](rust/) is the disposable
  Rust producer and executable reference. It is not Delta's semantic authority.

The lower-rung Delta-to-Gamma route under [`../omega-bootstrap/meaning/`](../omega-bootstrap/meaning/)
defines the meaning profile being widened across the canonical Delta compiler
and `omega-bootstrap`.
The former `compiler/delta` and `compiler/delta-rs` entries are retired; gates
use the canonical `delta` and `delta-rust` roles from `bootstrap/paths.sh`.
