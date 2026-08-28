# Target-bearing bridge compilation envelope, version 2

OMGCOMP2 is the first structural compilation-envelope profile allowed to carry
the source closure for one selected boundary-provider path. It reuses the
bounded package, source, alias, string, and nested-bundle representation from
[`OMGCOMP1`](OMEGA_BOOTSTRAP_COMPILATION.md) without reinterpretation. Its only
wire additions are a distinct major version and an exact selected-configuration
word.

This version binds the Linux x86-64 target and native-provider-substitution
configuration needed by the focused `Console::exit_process` carrier. It does
not resolve or validate `boundary trait`, `satisfies`, `via`,
`Binding::CompilerIntrinsic`, `select_provider`, or a field-receiver call.
Those spellings are opaque nested-bundle bytes here. In particular, OMGCOMP2
contains no trait, requirement, realization, binding, provider, provider-plan,
or admission IDs.

Structural validity grants no source, package, resolver, provider, lock, or
artifact authority. No accepted-lock or accepted-closure claim follows from
this envelope. A later resolver witness must prove the exact provider relation,
and external evidence must independently join the accepted closure and exact
envelope digest before compilation acceptance can be claimed.

## Inherited bounds and tables

OMGCOMP2 inherits all OMGCOMP1 integer, extent, canonicality, package graph,
source placement, alias, string, nested-bundle, exact-EOF, status, and
publication rules. The ceilings remain:

- 1 through 16 packages and source units;
- at most 32 aliases and 64 canonical strings;
- at most 131,072 content bytes per source and 262,144 in aggregate;
- at most 263,312 nested-bundle bytes; and
- at most 267,280 complete-envelope bytes.

Malformed, noncanonical, unsupported, or inconsistent input rejects with 251.
Exhaustion of a declared public ceiling selects 252 before later inspection can
downgrade it. The structural checker emits no bytes.

The package, source, alias, string, and nested-bundle tables are byte-for-byte
the OMGCOMP1 tables. Source labels remain custody and ordering metadata;
resolver-owned source rows remain the only logical module placements. Multiple
source extents may contribute to one module.

## Header -- 64 bytes

```text
offset  width  field
0       8      magic: ASCII "OMGCOMP\0"
8       u16    schema major: 2
10      u16    schema minor: 0
12      u16    target: 1 = Linux x86-64 System V
14      u16    flags: zero
16      u32    total envelope byte length
20      u32    nested bridge-source-bundle byte length
24      u32    canonical-string-table byte length
28      u32    canonical-string count
32      u32    package count
36      u32    source count
40      u32    alias count
44      u32    selected root package ID
48      u32    selected root source ID
52      u32    selected root owner-name string ID
56      u32    selected root machine-name string ID
60      u32    selected configuration: 1 = native provider substitution
```

There is no compatible-minor or compatible-configuration rule. OMGCOMP1
requires major 1 and word 60 equal to zero. OMGCOMP2 requires major 2 and word
60 equal to one. Thus each cross-version/configuration pair rejects, while all
existing OMGCOMP1 bytes retain their meaning.

The target and configuration say only which source closure orchestration asks
later stages to resolve. They do not select a realization by themselves and do
not authorize a compiler-known intrinsic.

## Focused carrier profile

The saved fixture contains exactly two packages and three source extents:

1. an application source in module `app`, with one direct `omega_std` alias;
2. a portable standard-library source in module `console`; and
3. a Linux x86-64 provider source also placed in module `console`.

The two standard-library files deliberately share one resolver-owned logical
module. This fixture therefore does not depend on unresolved private
cross-module visibility. The selected root is exactly `Main::main` in the
application source. The nested bytes spell one exact
`Console::exit_process(i32)` requirement, its bodyless Linux x86-64
`CompilerIntrinsic` realization, its sealed two-path provider selection, and a
final `self.console.exit_process(70)` call. OMGCOMP2 proves only custody of
those exact bytes and the two-package/three-source target-bearing graph.

Changing source bytes without changing lengths can remain structurally valid;
this is a required opacity control, not a semantic acceptance. Conversely,
wrong version/configuration pairs, wrong targets, malformed tables, graph
inconsistency, framing errors, and resource exhaustion are owned here.

The profile does not admit the complete product Console surface, other targets,
general generic calls, arbitrary provider families, provider trust, or final
checkpoint closure.

## Implementations and gate

[`omega_bootstrap_compilation_v2.py`](omega_bootstrap_compilation_v2.py) is the
deterministic pack/verify/inspect tool. It delegates unchanged table
canonicalization to the V1 owner and independently selects the V2 version and
configuration pair. [`omega-bootstrap-compilation-check.alp`](omega-bootstrap-compilation-check.alp)
shares the unchanged table checker between exact V1 and V2 headers.

The independent fixed-shape reference encoder and deterministic packer retain
the canonical fixture and inspection relation. The former producer wrapper
joined them byte-for-byte to native/self Delta checkers, exact 0/251/252 cases,
and the V1 regression. Producer replay is suspended until canonical Delta
publication.
