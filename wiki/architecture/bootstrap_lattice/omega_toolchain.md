# Psi/Omega toolchain

[Lattice overview](bootstrap_lattice.md) | [Delta language rung](rungs/delta.md)

Omega is the product-language endpoint, not another small bootstrap language.
Psi owns source processing through terminal portable IR; Omega consumes that IR
and performs target realization, optimization, and native emission. Today the
working implementations are primarily Rust. The hosted destination has one
profile-limited bridge compiler and one production compiler. Written as build
actions rather than artifact shorthand:

```text
language capability: Alpha → Beta → Gamma → Delta → Omega
```

The build inserts `omega-bootstrap` before the production artifact; that does
not insert another language into this progression:

```text
Delta compiler source ──[Delta→Gamma + Gamma execution]──▶ delta compiler
Delta bridge source ──[delta compiler]───────────────────▶ omega-bootstrap (accepts Ωself)
Ωself source ──[omega-bootstrap]──▶ omega (implements full Ω)
same Ωself source ──[optional omega]───▶ omega (same compiler, optimized binary)
```

The first line is the actual `Gamma → Delta` construction, not merely a
semantic side check. The canonical lower-rung route executes the Delta-written
compiler and publishes its native artifact. A later Delta self-rebuild is useful
reproducibility evidence, but the required path does not depend on the Rust
Delta producer.

`omega-bootstrap` is written in Delta and accepts only the compositional
Psi/Omega source surface required by the production source closure. It is
permitted to reject proof syntax, dependent or proof-indexed types, and any
other Omega construct excluded from `Ωself`. Accepted constructs retain exact Omega
semantics; this is not a bootstrap dialect. The production compiler is written
in Omega constrained to that `Ωself` profile and implements the full
specification for users.

Full specification coverage is a compiler property, not a tool-bundling rule.
Standalone Terminal-Psi interpreters, REPLs, proof explorers, viewers, and
debuggers are not bootstrap dependencies unless imported by the production
compiler executable.

The product compiler's own Terminal-Psi representation and lowering modules are
different: because the current ownership cut links them into the compiler, they
belong to its deterministic source closure as ordinary `Ωself` modules. The
Delta-written bridge only has to compile that source correctly. It need not
contain a Terminal-Psi interpreter or adopt Terminal Psi as its own internal IR;
a direct conservative checked-IR/lowering path is valid, and Terminal-Psi
validation becomes a bridge obligation only if that route is deliberately
selected.

Three separate properties are involved:

- `omega-bootstrap` is intentionally incomplete in Omega input coverage;
- it is exact, not approximate, for every program its published compositional
  profile admits; and
- the production `omega` it builds accepts and implements full Omega, even
  though that compiler executable may itself have been lowered conservatively.

These are the two bootstrap source questions: what Delta literally is, and
which ordinary Omega features the product compiler uses in its own source.
Full-Omega implementation coverage is already required by the product
specification, and conservative-versus-optimized generation is not a third
language surface.

The exact compiler-source manifest demonstrates closure under `Ωself`; it must
not become a whitelist of files, statement counts, or syntax-tree shapes.
The manifest yields a provisional profile. The profile freezes at the bridge
join, after the general implementation supplies real implementation and
assurance costs for retained features; source census alone cannot settle that
tradeoff.

The bridge binary may run slowly and lower the production compiler
conservatively. It must compile the `Ωself` source that implements the product
optimizer and advanced lowering, but need not duplicate or execute those passes
during the hosted build. The resulting production compiler does execute them
when compiling later inputs. A further
production `omega` → `omega` self-rebuild can optimize the compiler binary; it
is optional evidence, not a required dependency. The required bridge → product
edge is a cross-language hosted build, not that self-rebuild.

The distinction is architectural:

- Alpha, Beta, Gamma, and Delta form the small language chain used to build the
  bridge compiler from the audited seed. Delta is independent, not an Omega
  subset requirement.
- `Ωself` is a mechanically enforced Omega source profile, not Epsilon or
  another language rung.
- The bridge compiler builds the full-spec production compiler once from the
  exact `Ωself` source manifest, including its optimizer and advanced lowering;
  that compiler's own binary may initially be conservative.
- The Psi-aware artifact verifier reconstructs the obligations imposed by an
  exact terminal-Psi module; the [proof kernel](proof_kernel.md) independently
  checks the certificate derivations that discharge those obligations.

## Current repository roles

- `source/on-ramp/rust/{psi,omega}/` is the current working Rust
  compiler and executable reference producer;
  `source/on-ramp/rust/apps/omega-cli/` is its user-facing executable.
- `source/{psi,omega}/` owns the Omega-written product source. The complete Psi
  source-to-token spelling phase lives under `source/psi/`; later Psi phases
  and `source/omega/` remain open. Hosted product entrypoints live under
  `source/omega/`. Exact closure comes from Git, package resolution, and the
  accepted source closure rather than authored numbered snapshots.
- `source/on-ramp/omega-bootstrap/` is the owner for Rust-free meaning,
  Delta-written bridge-compiler slices/profiles, and bootstrap validation.
- `source/delta/` owns the bootstrap language corpus, Delta-written compiler
  experiment, and provisional artifacts. Its canonical lower-rooted compiler
  publication remains open; no external Delta producer is retained.

Role-local Rust bootstrap producers live beneath their rung; the current Rust
Psi/Omega compiler lives at `source/on-ramp/rust/`. Neither location grants
semantic authority. See the [repository structure](repository_structure.md).

Hosting does not by itself prove compiler correctness. A defect in
`omega-bootstrap` can reproduce while it builds production Omega. The value of
this shape is dependency closure: one checked hosted edge replaces a historical
tower of external implementation-language dependencies. Semantic correctness
still comes from the canonical meaning route, reconstructed proof obligations,
derivation checking, and translation validation across that edge.

The exact distinction between Delta's literal specification and `Ωself` is
defined in [`compiler_source_profile.md`](compiler_source_profile.md).

The current Rust `psi-terminal-verifier` demonstrates the artifact-aware half:
it validates canonical terminal Psi, reconstructs its exact obligation set,
rejects missing or extra evidence, and produces `VerifiedTerminalModule`. It is
not interchangeable with the generic proof kernel and remains an explicit
trusted migration dependency. The final hosted architecture uses one total low-
rung semantic-ledger definition over canonical bytes. Direct evaluation or a
checked derivation of that definition establishes every deployed artifact's
ledger; Rust agreement grants no authority. Local operation denotations and
canonical goals come from restricted declarative schemas, while algebraic
reduction is untrusted and must emit a checked proof of the unchanged goal.

Bridge hosting and the one required production compile are tracked in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md). Native refinement evidence
and terminal-ledger migration remain product-assurance work under P3 in
[`TASKS.md`](../../../TASKS.md).
Production optimization remains outside the trusted proof kernel.

The closed O0/O1 console and bounded scalar-call tranches remain bridge
regression canaries. They establish useful source-to-terminal, direct-ELF,
resource, self-built, and lower-rung-meaning seams, but they are not numbered
compiler generations and admit no feature to `Ωself`. Their exact contracts,
limits, observations, and remaining target boundaries live beside the bridge in
[`BOOTSTRAP_PROFILES.md`](../../../source/on-ramp/omega-bootstrap/compiler/BOOTSTRAP_PROFILES.md)
and the [bridge README](../../../source/on-ramp/omega-bootstrap/README.md). New
architecture should be derived from the complete source contracts above, not
from those historical canary envelopes.
