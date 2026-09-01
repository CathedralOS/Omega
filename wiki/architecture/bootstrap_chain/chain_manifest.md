# Bootstrap chain manifest

[Chain overview](bootstrap_chain.md) | [Active work](../../../TASKS_BOOTSTRAP.md)

This is the audit ledger for the one permitted compiler chain. A row names an
exact source subject, its compiler artifact, and the checked source-to-Alpha-tape
edge. Diagnostics may expose bugs, but they are never extra compiler stages.

```text
audited Alpha VM seed
  → direct Beta assembler tape
  → Beta-written Gamma compiler       → gamma_compiler_bytecode.tape
  → Gamma-written Delta compiler       → delta_compiler_bytecode.tape
  → Delta-written Epsilon compiler      → epsilon_compiler_bytecode.tape
  → Epsilon-written full Omega D        → omega0_compiler_bytecode.tape
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
| Beta assembler | Alpha tape | exact Beta source reconstructs the direct assembler tape | canonical raw tape and self-host source exist; byte-for-byte reconstruction is executable |
| Gamma compiler | Beta | exact Beta source refines the exact Gamma-compiler tape | canonical source and direct artifact exist; full refinement and resource outcomes remain open |
| Delta compiler | Gamma | exact Gamma source refines the exact Delta-compiler tape | D16 fixes the language, D19 its logical application profiles, D20 its resolver identity/scope, D21 its `Bytes` length invariant, D23 its coherent one-MiB Alpha profile, D30 its physical request/outcome profiles, D33 bounded request/schema diagnosis, and D58 the complete compiler's measured conjunctive Gamma resource profile; incomplete source owns resolved lowering, whole-function emission, and exact D19 source-schema/bijection validation, while adapter emission, D58 measurement/publication, tape, and refinement remain open |
| Epsilon compiler | Delta | exact Delta source under `EpsilonCompilerV1` refines the exact Epsilon-compiler tape | D17 fixes Epsilon v1 and its `ECOUT` boundary, D22/D24/D36 fix declaration and callable identity, D31 fixes structural type formation, D34 fixes bounded-witness storage refusal, D37 fixes body/control premise composition, D38 fixes `.as_slice`, D50 fixes bare-state transfer spelling, D51 requires receivers on qualified data machines while making reserved `self` an ordinary binding, D52 fixes resultless-argument anchoring, D53 fixes local block exits without reachability analysis, D56 fixes entry diagnostics, and D57 fixes transition-pattern/coverage diagnostics; incomplete source owns parsing, census, catalog, local/value/place facts, call/state/transition facts, and symbolic encoding, but still contains D51-superseded receiverless qualified-machine, case/machine-collision, direct-static-call, and special-self machinery. D50/D51/D52/D53/D56/D57 implementation, remaining D37/D38 work, storage, lowering, tape, executable suite, and refinement remain open |
| `omega₀` | Epsilon closure `D` | exact `D` refines a full Omega compiler represented as Alpha tape | D18 fixes the logical request, D25 its outer/canonical `OCREQ`/`OCOUT` rules, and D59 the flat inner-profile and bounded-publication rules; `D` owns the outer envelope plus lexical, parser, and Alpha-encoder slices, while the checked numeric tables, full compiler, and edge remain open |
| `omega` | Omega closure `C` | exact `C`, compiled by `omega₀`, refines a full Omega compiler represented as Alpha tape | D18, D25, and D59 fix the common standalone edge; its checked inner wire/failure tables and product source remain incomplete |

No later fixed point repairs an open earlier row. Every row must stand on its
own exact source, exact tape, source semantics, Alpha semantics, observation
profile, checked derivation, and disclosed realization admissions.

Every Beta-assembly, Gamma, Delta, or Epsilon source subject in those rows also
conforms to D15's closed textual-ASCII envelope. The closure retains original
bytes and byte coordinates; no decoder, Unicode table, locale, or filename
extension participates in source recognition.

## Alpha execution floor

Current committed native seeds:

| target | artifact | SHA-256 |
| --- | --- | --- |
| Darwin arm64 | `source/alpha/alpha_arm64_macos` | `5844f295e3ab843e1819aae0ca47d41ad99cef5e2193a5abee64e630b41c304c` |
| Windows x86-64 | `source/alpha/alpha_x64_windows.exe` | `ccce78bbef7cb5a538d4fb0e350a1c646233d179074e65b4ac7cca98c4a4a6f7` |

The accepted compiler artifact above this floor is always the raw Alpha tape.
Transparent seed stamping prepends the tape length inside the disposable host
container; that prefix and container are not compiler identity. The Beta
assembler is the first language rung. The derivation checker remains an
Alpha-owned service beside the language chain.

## Migration evidence that may be retained

- `source/gamma/compiler/gamma_compiler.beta` is the canonical Beta-written
  Gamma compiler. Its direct assembly is the persisted tape consumed above.
- `source/delta/compiler/delta_compiler.gamma` is the canonical incomplete
  Gamma-written Delta compiler source; its strict frontend and direct Alpha
  emitter substrate do not become an edge until lowering, the adapter, tape,
  and refinement exist. `tests/delta/interpreter/interp.gamma` remains a bounded oracle,
  and oracle agreement cannot establish a rule both compared paths omit.
- the restricted Epsilon-written Darwin compiler prototype was deleted. The real
  `D` is authored under Omega ownership rather than inheriting a compiler-shaped
  historical monolith.

The former Gamma-written Epsilon-to-Delta bridge and Darwin-native publication
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

- representative `epsilon → omega₀` or `omega₀ → omega` work has unacceptable
  wall time, memory use, or tape size after ordinary profiling and cleanup;
- Alpha's current instruction set or encoding appears too verbose, creating
  pressure for a new opcode, wider encoding, or smuggled high-level primitive;
- proof size or checker time explodes after DAG sharing and compositional lemmas;
- useful execution seems to require a source-, function-, hash-, or
  workload-specific native substitution (a jet);
- a target ABI, object format, linker, runtime, or host compiler begins leaking
  into Gamma, Delta, Epsilon, or canonical compiler identity;
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
