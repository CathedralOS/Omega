# The Bootstrap Chain

> **Status: fixed spine, surfaces under minimization, incomplete upper
> implementations.** The language spine is
> `Alpha → Beta → Gamma → Delta → Epsilon → Omega`. Alpha tape is the canonical
> executable form of every bootstrap compiler. The existence and ordering of
> the rungs are fixed; their instruction sets, language features, compiler
> boundary protocols, and checker calculus are not presumed minimal. Current
> Gamma has its canonical immediate-predecessor source and direct tape; its full
> refinement remains open, as do the upper compiler implementations.

## The chain

Let `D` be the exact Epsilon-written source closure of the first full Omega
compiler, and `C` the exact Omega-written source closure of the self-hosting
compiler:

```text
audited Alpha seed
  → direct Beta assembler tape
  → Beta-written Gamma compiler       → gamma_compiler_bytecode.tape
  → Gamma-written Delta compiler       → delta_compiler_bytecode.tape
  → Delta-written Epsilon compiler      → epsilon_compiler_bytecode.tape
  → Epsilon-written Omega closure D     → omega0_compiler_bytecode.tape
  → Omega-written Omega closure C     → omega_compiler_bytecode.tape
```

Every compiler consumes exactly one language and emits one platform-independent
Alpha tape. No lower rung parses past its immediate successor, and no external
historical compiler remains necessary after the next tape exists. `omega₀` and
`omega` implement the same full Omega language, but they come from different
implementation-language source closures. `omega₀` may be slow and
conservatively generated; `omega` may apply the optimizer implemented by the
Omega source.

The former Gamma-written Epsilon-to-Delta translator crossed two ownership
boundaries and is deleted with its native-publication apparatus. Git history is
the archive; no compatibility route replaces it.

## Minimize the complete chain

Each intermediate language primarily exists to express one exact successor;
Gamma additionally expresses the root checker. Present implementation and
corpus use establish migration cost, not language merit. Every instruction,
construct, failure distinction, profile, wire field, proof rule, and sidecar
must justify its cost against the complete chain rather than resemble a feature
a conventional general-purpose language or public compiler service would have.

The selected objective balances semantic size, native/trusted implementation,
successor source and tape size, proof/certificate burden, mutable state,
checking work, and permanent validation/tooling. Lower-rung complexity is
weighted more heavily because every later edge inherits it. Saving source bytes
by adding an opaque compound opcode or proof axiom is not minimization.

The active method and completion criteria live in
[Bootstrap minimization](../../design_briefs/bootstrap_minimization.md). The
ordered audit and implementation tasks are in `TASKS_BOOTSTRAP.md`. The audit
works backward from exact successor sources, selects one whole-chain design,
then implementation and artifact reconstruction proceed from the lowest
changed rung upward.

Beta assembly, Gamma, Delta, and Epsilon implementation source also share the
closed textual-ASCII envelope fixed by [D15](decisions.md#d15--bootstrap-implementation-source-is-closed-textual-ascii).
This removes host decoding, Unicode tables, normalization, and invisible
control-byte trivia from the bootstrap trust surface. Arbitrary bytes remain
ordinary input, output, and artifact data rather than raw implementation-source
characters.

## One canonical executable representation

Alpha tape is the bootstrap authority from Gamma through `omega`. A
target-specific Alpha VM seed executes the same tape on every supported host.
Stamping the tape into a Mach-O, ELF, or PE seed is transparent packaging; the
container, signature, and installation are not new compiler artifacts.

This avoids requiring Gamma, Delta, or Epsilon to implement hardware-specific
backends. Each needs one lowering to the small Alpha machine. The product Omega
compiler still owns native target backends for user programs.

An optional general Alpha-to-native realization may improve operational speed
only when checked against Alpha semantics. It must translate arbitrary Alpha
tapes uniformly. Source-specific accelerators, function substitutions, tape-hash
shortcuts, and other jets are outside the canonical chain.

## Immediate-predecessor ownership

“Compiler for language L” and “source written in language L” are different
facts. The canonical source relationship is:

| Compiler artifact | Implementation source | Accepted input | Canonical output |
| --- | --- | --- | --- |
| Beta assembler | Alpha tape | Beta | Alpha tape |
| Gamma compiler | Beta | Gamma | Alpha tape |
| Delta compiler | Gamma | Delta | Alpha tape |
| Epsilon compiler | Delta | Epsilon | Alpha tape |
| `omega₀` | Epsilon (`D`) | Omega | Alpha tape |
| `omega` | Omega (`C`) | Omega | Alpha tape |

A self-hosted implementation of an intermediate language may remain a test,
optimization experiment, or later replacement. It is not an extra required
edge and cannot displace the immediate-predecessor construction merely by
reproducing itself.

## Trust by checking, not pedigree

For every compiler artifact, Omega fixes and checks:

```text
exact source subject + exact Alpha tape
  + canonical source semantics + Alpha semantics
  + observation and resource profiles
  + reconstructed obligations + certificates
  + disclosed admissions
  → checked source-to-tape refinement claim
```

The producer does not choose the obligation set, semantics, or observation
profile. A verifier result is a re-derivable cache, not authority in itself.
Producer identity and reproducibility remain useful operational metadata but do
not enter the semantic verdict.

Common Alpha targeting makes the artifact side of every proof identical: one
tape decoder, one small-step machine, one memory model, and one terminal-event
vocabulary. Source parsing and source semantics remain language-specific.
Compositional proof IRs may reduce certificate size, but they are checked
lemmas—not executable bridge stages.

## Five roles often confused as “the bottom”

1. **Seed execution** realizes Alpha semantics on a physical host.
2. **Language semantics** define Alpha, Gamma, Delta, Epsilon, and Omega.
3. **Compiler construction** produces the next Alpha tape.
4. **Proof checking** validates derivations independently of their producers.
5. **Admissions** disclose hardware, firmware, foreign-system, and release
   claims that no formal edge closes.

No implementation gains authority by occupying more than one role.

## The fixed language spine

| Rung | Responsibility | Canonical implementation direction |
| --- | --- | --- |
| [Alpha](rungs/alpha.md) | minimal deterministic tape execution | written semantics plus audited native VM seeds |
| [Beta](rungs/beta.md) | textual Alpha assembly | direct Alpha tape plus readable self-host source |
| [Gamma](rungs/gamma.md) | small structured compiler language | Beta-written compiler to Alpha tape |
| [Delta](rungs/delta.md) | safe typed definitional computation | Gamma-written direct compiler to Alpha tape |
| [Epsilon](rungs/epsilon.md) | deterministic compiler-host systems language | Delta-written compiler to Alpha tape |
| [Omega](omega_toolchain.md) | full product language and compiler | Epsilon-written first compiler, then Omega-written compiler |

The Alpha-owned [proof kernel](proof_kernel.md) is universal checker
infrastructure, not another rung. Terminal Psi is an internal product compiler
boundary, not a language rung. The Rust compiler is a comparator and working
implementation, not part of the canonical chain.

## Orchestration is replaceable

`tools/bootstrap/` may invoke a compiler, stamp a tape, compare artifacts, and
report diagnostics. It may not discover source closure, parse or lower accepted
source, manufacture evidence, or make a trust decision. Deleting or rewriting a
runner may change ergonomics; it must not change chain meaning.

Host-language reference implementations are development scaffolding with
explicit deletion conditions, not a permanent diversified implementation rung.
The completed repository can construct and check the chain from its audited
native Alpha seed and owned bytes without Python, Rust, a network, a package
manager, or a host Unicode database.

## The repository is the chain, not its history

Owned machinery outside the direct chain is a liability even when it is
technically functional. It increases the code and proof surface that must be
understood, gives obsolete architectures apparent legitimacy, and taxes every
future change with irrelevant tests. Retention therefore requires a current
canonical owner, an exact edge property, and a cheaper-than-replacement way to
detect a named failure. Migration code additionally requires a deletion
condition. Code that cannot meet those requirements is removed rather than
parked, generalized, or preserved as an alternate route.

## Owner escalation criteria

Work stops for an owner ruling when implementation pressure indicates that the
architecture itself may be wrong:

- terrible wall time, memory use, or tape size on representative
  `epsilon → omega₀` or `omega₀ → omega` work;
- excessive Alpha verbosity or pressure for new instructions/encodings;
- prohibitive proof size or checking time after compositional cleanup;
- apparent need for jets or other special native substitutions;
- target-specific behavior leaking above Alpha or below product Omega;
- inability of one compiler to emit the next tape without older compilers or
  semantic scripts;
- realistic closures crossing private bounds or undefined Alpha behavior;
- pressure for a new trusted proof axiom rather than better proof production;
- disagreement between canonical VM execution and optional native realization;
  or
- a retained legacy component requiring a second accepted chain, duplicate
  source of truth, or permanent compatibility adapter.

The trigger opens a design question. It does not authorize a local workaround.

## No diversified-compilation stage

Diversified double compilation is not a rung or release requirement. The
audited seed plus direct checked source-to-tape refinement at every edge address
compiler corruption across the whole chain. Independent compilers and checkers
remain useful regression oracles, but agreement is diagnostic only.

## Open work

The ordered work is in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md). The principal open edges are:

- finish admission and exact source-to-tape refinement for the already
  canonical Beta-written Gamma construction;
- turn the Gamma-written Delta implementation into the compiler artifact needed
  to consume Delta and emit Alpha tape;
- implement the Epsilon compiler in Delta;
- author the first Omega compiler source closure `D` in Epsilon under Omega
  ownership, using historical prototypes only as Git-resident reference;
- build `omega0_compiler_bytecode.tape` from `D`; and
- compile `C` with `omega0` into `omega_compiler_bytecode.tape`.

The exact admitted and missing subjects are summarized in the
[bootstrap chain manifest](chain_manifest.md).
