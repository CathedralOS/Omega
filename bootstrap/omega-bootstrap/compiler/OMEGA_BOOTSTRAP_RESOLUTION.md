# Omega-bootstrap normalized resolution handoff

[`OMEGA_BOOTSTRAP_COMPILATION.md`](OMEGA_BOOTSTRAP_COMPILATION.md) |
[`OMEGA_BOOTSTRAP_CHECKED_IR.md`](OMEGA_BOOTSTRAP_CHECKED_IR.md) |
[`OMGCOMP refinement`](../../assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS.md)

This contract fixes the bridge-private boundary between multi-unit Omega source
resolution and checked-IR lowering:

```text
exact OMGCOMP
    │
    └── omega-bootstrap-resolve ──▶ canonical OMGRSW1

exact OMGCOMP + exact OMGRSW1
    │
    └── omega-bootstrap-resolved-to-ckir ──▶ CKIR
```

`OMGRSW1` is the normalized frontend/resolution handoff. It retains source
units, imports, every non-builtin static binding, semantic-order declarations,
normalized types and signatures, exact body spans, and the selected machine.
It deliberately contains no body operations. The exact row schemas, ordering,
relations, and version-1 ceilings are specified in
[`OMGCOMP_REFINEMENT_WITNESS.md`](../../assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS.md#omgrsw1-header--72-bytes).

The same exact bytes have two roles without acquiring authority from either:

- in the compiler pipeline they are the bridge-private normalized frontend
  handoff; and
- inside `OMGRFN2` they are an untrusted witness independently reconstructed by
  lower-rooted checkers.

This is not Omega syntax, backend checked IR, a resolver/lock receipt, a stable
product ABI, or a trust grant. CKIR remains the sole input to the native backend.

## Responsibility split

`omega-bootstrap-resolve` owns:

- independent lexing within every resolver-owned source extent;
- unique authored `module` agreement with resolver placement;
- semantic package/source/authored declaration order;
- requester-local direct aliases and exact `use` paths;
- alias-versus-same-package-top-level-module ambiguity;
- declaration namespaces, duplicate rejection, and visibility;
- every non-builtin static binding and normalized source type;
- exact selected-root lookup within the selected package, source, and module;
  and
- canonical `OMGRSW1` publication only after the complete result fits.

It does not lower bodies, read or emit CKIR/ELF, accept a package receipt, or
compare SHA-256. Structural `OMGCOMP` validity and the external receipt/digest
join remain separate conjuncts.

`omega-bootstrap-resolved-to-ckir` owns:

- safe decoding of the exact paired input below;
- local validation of every witness extent, ID, source span, and relation it
  consumes;
- copyability, by-value acyclicity, layout, and CKIR type interning;
- reparsing exact source bodies using resolved witness identities;
- operations, values, places, terminators, transition facts, and result
  reconstruction; and
- canonical CKIR publication only after the complete artifact fits.

It does not redo package/name resolution. Independent source-to-witness and
witness/source-to-CKIR checkers establish those assurance conjuncts.

The existing `omega-bootstrap-source-custody-check.alp` remains the frozen
one-unit regression/reference producer. Multi-unit production work must not
turn it into the resolver, lowerer, and artifact checker at once.

## Lowerer input frame — `OMGLOW1`

The resolver emits only `OMGRSW1`. Untrusted orchestration pairs those bytes
with the unchanged compilation envelope in this exact little-endian frame:

```text
offset  width  field
0       8      magic: ASCII "OMGLOW1\0"
8       u16    schema major: 1
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP length
24      u32    exact OMGRSW1 length
28      u32    reserved: zero
32      ...    exact OMGCOMP || exact OMGRSW1 || exact EOF
```

There is no compatible-minor-version rule. All integers fit signed 32-bit
bootstrap arithmetic. Checked multiplication/addition precedes every offset.
With the current component ceilings, the maximum frame is
`32 + 267,280 + 524,288 = 791,600` bytes.

Malformed framing, unsupported versions, relation failures, truncation, or
trailing bytes return 251 without output. A declared component or output
ceiling exceeded by otherwise valid input returns 252 without output. Once an
extent selects 252, later inspection may not downgrade it to 251.

`OMGLOW1` carries no digest, receipt, CKIR, ELF, claimed result, or authority.
The lowerer locally checks enough `OMGCOMP` and `OMGRSW1` structure to make all
accesses safe and to bind:

- witness unit/package/module IDs to the exact envelope rows;
- every source span to the exact independently delimited source content;
- declaration, type, record, field, machine, parameter, and block partitions;
- every binding to its declaration/import and source extent; and
- the selected witness machine to the exact envelope root.

The later `OMGRFN2` assurance frame embeds the exact same `OMGCOMP` and
`OMGRSW1` bytes unchanged beside CKIR and ELF.

## Why source flattening is not the boundary

A one-unit flattened source happens to work as a fixture oracle because its
declarations have unique short names. It is not the production architecture.
Legal same-spelled declarations in different packages/modules would require a
binding-aware alpha-rewriter for every declaration and static reference;
flattening also distorts source custody and loses the exact selected root unless
it recreates the complete resolver. `reference-flat.omg` therefore remains
untrusted expected-output plumbing only.

## CKIR1 root boundary

`OMGCOMP` and `OMGRSW1` select one exact machine by package, source, module,
owner, and machine. CKIR1's current source producer additionally requires that
the complete source have exactly one zero-explicit-parameter scalar-result
candidate. The first two-unit fixture satisfies both rules, so it may close
without changing CKIR1.

Do not prune unrelated machines or silently weaken CKIR1 to manufacture that
property. Before admitting a compilation with another candidate-shaped
machine, version CKIR. The successor must carry an explicit entry machine ID,
validate its conformance signature without a global candidate-cardinality rule,
and bind that ID to the exact `OMGCOMP`/`OMGRSW1` root in lower-rooted evidence.

## First implementation tranche

The first resolver/lowerer milestone supports the general bounded call-free
nominal-data surface already represented by CKIR1:

- nonempty and package-root logical modules;
- exact `module`, `use`, and `pub data` handling;
- direct cross-package and same-package data imports;
- records, fields, normalized nominal/scalar/array types, attached-machine
  signatures and states, and exact body spans;
- every static nominal binding and the exact selected root; and
- the existing finite, acyclic, returning body/lowering profile.

Cross-unit machine calls remain reserved until a versioned CKIR call operation
lands. Same-package private access across distinct modules is not admitted until
the language visibility rule is specified. A final-name collision between an
import and a same-module declaration fails closed in this tranche. Neither gap
blocks the public cross-package nominal-data artifact.

Every implementation milestone carries phase-isolated semantic negatives,
exact/adjacent resources, deterministic output, native and Delta-self-built
agreement, Rust-free meaning observations, and lower-rooted cross-pairs. The
version-1 capacities are private implementation/evidence limits rather than
`Ωself` language limits; widening versions the carrier instead of silently
changing its meaning.
