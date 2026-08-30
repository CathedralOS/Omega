# Bootstrap chain manifest

[Lattice overview](bootstrap_lattice.md) | [Active work](../../../TASKS_BOOTSTRAP.md)

This is the audit ledger for the one permitted compiler chain. A row names an
exact source subject, its compiler artifact, and the checked source-to-Alpha-tape
edge. Diagnostics may expose bugs, but they are never extra compiler stages.

```text
audited Alpha VM seed
  → Alpha-written Beta compiler       → beta_compiler_bytecode.tape
  → Beta-written Gamma compiler       → gamma_compiler_bytecode.tape
  → Gamma-written Delta compiler      → delta_compiler_bytecode.tape
  → Delta-written full Omega D        → omega0_compiler_bytecode.tape
  → Omega-written full Omega C        → omega_compiler_bytecode.tape
```

## Retention rule

Repository-owned code and evidence must directly do at least one of these:

1. specify a rung;
2. implement its immediate-predecessor compiler;
3. prove or reconstruct that compiler's exact source-to-tape edge; or
4. exercise a focused semantic or performance property of that edge.

Anything that cannot be adapted into one of those roles is removed. Retention
is not neutral: every extra component enlarges the audit surface, consumes test
time, and obscures the single supported architecture. Git history is the
archive. In particular, a lower rung parsing past its successor, a host script
supplying compiler semantics, a native compiler artifact above Alpha, or a
receipt ceremony without a semantic edge has negative maintenance value.

## Required obligations

| Canonical subject | Source language | Required obligation | Current state |
| --- | --- | --- | --- |
| Alpha VM seed | native assembly | audited realization of `source/alpha/SEMANTICS.md` | Darwin arm64 and Windows x64 seeds exist; human binary/listing and physical-platform admissions remain irreducible |
| Beta compiler | Alpha | exact Alpha source refines the exact Beta-compiler tape | canonical source and direct artifact exist; full refinement and resource outcomes remain open |
| Gamma compiler | Beta | exact Beta source refines the exact Gamma-compiler tape | D16 fixes the language and compiler boundary; standalone compiler source, tape, and refinement remain open |
| Delta compiler | Gamma | exact Gamma source refines the exact Delta-compiler tape | D17 fixes Delta v1 and the compiler boundary; source, tape, suite, and refinement remain open |
| `omega₀` | Delta closure `D` | exact `D` refines a full Omega compiler represented as Alpha tape | `D` is not yet authored; the obsolete native prototype was deleted |
| `omega` | Omega closure `C` | exact `C`, compiled by `omega₀`, refines a full Omega compiler represented as Alpha tape | product source is incomplete; edge is open |

No later fixed point repairs an open earlier row. Every row must stand on its
own exact source, exact tape, source semantics, Alpha semantics, observation
profile, checked derivation, and disclosed realization admissions.

Every Alpha-assembly, Beta, Gamma, or Delta source subject in those rows also
conforms to D15's closed textual-ASCII envelope. The closure retains original
bytes and byte coordinates; no decoder, Unicode table, locale, or filename
extension participates in source recognition.

## Alpha execution floor

Current committed native seeds:

| target | artifact | SHA-256 |
| --- | --- | --- |
| Darwin arm64 | `source/alpha/alpha_arm64_macos` | `e3bb2be7c9e40b3c7a0e66c98568194a743d6d6e354d467386e222ef35dde927` |
| Windows x86-64 | `source/alpha/alpha_x64_windows.exe` | `0b8c3bb6d374d5a7a03de1e16be1f7206248acae990c2594a040291c7c866cb2` |

The accepted compiler artifact above this floor is always the raw Alpha tape.
Transparent seed stamping prepends the tape length inside the disposable host
container; that prefix and container are not compiler identity. The Alpha
assembler and derivation checker are Alpha-owned services; they are not
language rungs.

## Migration evidence that may be retained

- `source/beta/compiler/beta_compiler.alpha` is the canonical Alpha-written
  Beta compiler. Its direct assembly is the persisted tape consumed above.
- `source/gamma/compiler/gamma_compiler.beta` is the canonical incomplete
  Beta-written Gamma compiler source; its strict frontend and direct Alpha
  emitter substrate do not become an edge until lowering, the adapter, tape,
  and refinement exist. `source/gamma/interp.beta` remains a bounded oracle,
  and oracle agreement cannot establish a rule both compared paths omit.
- the restricted Delta-written Darwin compiler prototype was deleted. The real
  `D` is authored under Omega ownership rather than inheriting a compiler-shaped
  historical monolith.

The former Beta-written Delta-to-Gamma bridge and Darwin-native publication
apparatus are deleted. They crossed an immediate-predecessor boundary and
created a noncanonical compiler identity; no successor task may recreate them.

## Conditional diagnostics

Fixed-point reproduction, Rust agreement, multiple VM agreement, fuzzing,
viewers, timing reports, native-container reproduction, and repeated executions
may be useful. None can replace or add a premise to the six required obligations
above. A diagnostic remains only while it has a canonical owner, bounded cost,
a specific failure it reveals on the current canonical subject, and a deletion
condition. Duplicate coverage, historical interest, or hypothetical future use
requires deletion rather than indefinite retention.

Python implementations are necessarily temporary members of this category.
They are removed as checked direct edges subsume their named comparisons and do
not survive into the completed offline bootstrap closure.

## Owner escalation

Stop and open an owner ruling before changing the architecture when any of the
following occurs:

- representative `delta → omega₀` or `omega₀ → omega` work has unacceptable
  wall time, memory use, or tape size after ordinary profiling and cleanup;
- Alpha's current instruction set or encoding appears too verbose, creating
  pressure for a new opcode, wider encoding, or smuggled high-level primitive;
- proof size or checker time explodes after DAG sharing and compositional lemmas;
- useful execution seems to require a source-, function-, hash-, or
  workload-specific native substitution (a jet);
- a target ABI, object format, linker, runtime, or host compiler begins leaking
  into Beta, Gamma, Delta, or canonical compiler identity;
- a compiler cannot consume its language and emit the next runnable tape
  without an older rung, semantic interpreter, or host translation script;
- realistic compiler source crosses an unstated capacity, depends on undefined
  behavior, or cannot fail closed on resource exhaustion;
- proof completion appears to require a new trusted axiom or kernel rule rather
  than a better untrusted producer or reusable checked lemma;
- two conforming Alpha realizations disagree on the same exact tape/input; or
- retaining a legacy component requires describing a second accepted chain.

These conditions authorize investigation and an owner question, never a local
workaround that silently changes the chain.
