# Delta rung

This directory canonically owns the Delta language corpus, the current
Delta-written self-hosting/reference compiler experiment, and lattice-built
Delta artifacts. Architectural ownership is independent of the language used
to produce an artifact. The final Delta program on the hosted path is
`omega-bootstrap`, which accepts `Ωself` and builds the full production Omega
compiler.

Delta is an independent compiler-host language, not an Omega subset. Its v1
contract is being discovered from the complete `omega-bootstrap` source closure
plus explicit coherence, safety, robustness, and maintainability arguments, and
is tracked in
[`../../../TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

The corpus and disposable Rust producer are discovery inputs, not a feature
vote. A construct belongs to Delta v1 when the bridge demonstrates a concrete
need or an explicit language-coherence, robustness, safety, or maintainability
argument shows that retaining it reduces whole-bootstrap cost; its lower-rung
meaning must also close. Shared constructs should retain Omega spelling and
ordinary meaning where that is cheap. Source outside Delta v1 rejects as Delta
source; rejection of Omega outside `Ωself` is `omega-bootstrap`'s separate
responsibility. The working host surface is sealed byte input, artifact output,
diagnostic output, and process termination rather than a general boundary-trait
system.

- [`samples/`](samples/) contains the executable language corpus.
- [`FEATURE_LEDGER.md`](FEATURE_LEDGER.md) tracks provisional candidates and the
  evidence required to retain or remove them before the v1 freeze.
- [`samples/lowermachine.alp`](samples/lowermachine.alp) is the Delta-written
  Delta-to-ARM64 compiler and self-host fixed point. Its native publisher
  batches `write_byte` output in a fixed 4 KiB buffer, flushes before ordered
  line output/input, explicit or implicit entry return, and traps, and retains
  direct regression teeth at the buffer boundary; this is sealed host I/O, not
  a general allocator or runtime.
- [`build/`](build/) contains the checked-in bootstrap compiler artifacts.
- [`../../onramps/delta-rust/`](../../onramps/delta-rust/) is the disposable
  Rust producer and executable reference. It is not Delta's semantic authority.

The lower-rung Delta-to-Gamma route under [`../../omega-bootstrap/meaning/`](../../omega-bootstrap/meaning/)
defines the meaning profile being widened toward `omega-bootstrap`.
The former `compiler/delta` and `compiler/delta-rs` entries are retired; gates
use the canonical `delta` and `delta-rust` roles from `bootstrap/paths.sh`.
