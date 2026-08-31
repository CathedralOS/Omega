# Rung: Delta — compiler-host systems language

[Lattice overview](../bootstrap_lattice.md) | Prev: [Gamma](gamma.md) | Next:
[Omega product toolchain](../omega_toolchain.md)

Delta is the robust C-like implementation language used to write the first full
Omega compiler. It is independently specified, not an Omega subset and not a
Gamma macro surface.

## Direct responsibility

```text
Gamma-written Delta compiler source
  └─ gamma_compiler.tape ─▶ delta_compiler_bytecode.tape

Delta-written Omega compiler source D
  └─ delta_compiler.tape ─▶ omega0_compiler_bytecode.tape
```

The Delta compiler accepts Delta and emits Alpha tape. The Delta-written program
`D` accepts Omega and is the first full Omega compiler. Calling both artifacts
“the Delta compiler” obscures this boundary and is forbidden in current docs.

## Language boundary

Delta provides deterministic state-machine control, checked scalar arithmetic,
finite aggregates, fixed storage or explicit allocation failure, sealed byte
I/O, and enough modularity to maintain `D`. It does not inherit Omega's proof
surface, dependent types, packages, optimizer, or target model merely because
`D` implements those facilities for Omega users.

D17 and `source/delta/LANGUAGE.md` fix the exact v1 surface. Checking and
execution are separate judgments; `Incomplete` is an outer tool outcome, not a
Delta program result. `D` uses source-declared fixed arrays and integer indexes
for dynamic compiler structures because v1 has no heap or recursive value
types.

D22 fixes declaration identity and lexical scope: grammar-selected namespaces,
one pre-type scoped duplicate census, no active local shadowing, disjoint state-
local reuse, and categorical `InvalidBoundary` rejection for authored machine
bodies on boundary owners. D24 completes the same census for transition payload
binders, disjoint arm scope, and declaration-start ordering between
`DuplicateName` and `InvalidBoundary`; owner kind is never inferred from an
ambiguous owner row.

D31 fixes type formation and D34 fixes its realizability report: arrays admit
`1..INT32_MAX`, empty data is one zero-field record, mixed data rejects,
`never`/view/`Console` placement has one
structural reason and coordinate, and traversal never chooses a diagnostic.
Valid source that exceeds one selected application-static-storage profile
produces deterministic attributed or aggregate outer `Incomplete`, using an
exact demand while representable and `INT64_MAX` as the canonical larger-
demand witness, never a Delta rejection.

The canonical compiler source now implements complete parsing, that identity
census, D31's profile-independent structural type formation, a source-backed
resolution catalog, ordered local resolution, scalar/aggregate value-place
facts, and pure symbolic Alpha encoding. The formed program
retains explicit record/sum classification and direct value-containment edges;
the catalog keeps one row per top-level declaration and exact owner/member/state
lookups without flattening the AST again. Candidate selection covers array
length, shape, placement, unknown owners, and recursive value cycles by exact
source coordinate. Ordered body scans retain exact machine/state parameter,
let, and arm-binder identities, enforce pending-let visibility, and isolate
entry, state, and arm environments without selecting a final rejection. The
same walk retains exact-AST facts for settled literal, local-read, group,
negation, binary scalar, record-field, array/view index, range-slice, and
`.len` forms without duplicating the recursive checker. Exact field custody
preserves its owner/member identity for later layout and lowering.
Remaining body/control checking, lowering, `main`, tape
publication, and refinement are open implementation work. Q4 blocks the
incomplete entry-diagnostic judgment. D36's qualified-only receiver parser and
case/machine collision census are implemented; its qualified-expression
application classification remains open. D37 settles dependent parent
composition and relational statement/projection anchors; its full body/control
enforcement remains implementation work. Q6 blocks only the accepted receiver
set for `.as_slice`. D34 has settled physical storage refusal; its
implementation additionally waits on the final nonaliasing map.

Every source-visible bound, resource-profile parameter, and private
implementation budget is distinguished. Private exhaustion returns
`Incomplete` and publishes no tape.

## Implementation owners

- `source/delta/LANGUAGE.md` owns Delta syntax and semantics;
- the target compiler source is `source/delta/compiler/delta_compiler.gamma`;
- `source/delta/compiler/delta_compiler_bytecode.tape` is the future
  canonical artifact; and
- adjacent validation owns Gamma-source/Alpha-tape refinement.

The former `source/delta/meaning/delta2gamma.beta` route and the restricted
Delta-written Darwin compiler prototype are deleted. Neither implemented the
Gamma-written Delta edge or a full Omega `D`; their historical source remains
available in Git without occupying a live compiler owner.

## Closure criteria

Delta closes when:

1. its independent D17 language contract remains complete;
2. a Gamma-written compiler accepts that language and emits exact Alpha tape;
3. the tape directly refines the Gamma compiler source under Gamma and Alpha
   semantics;
4. the compiler accepts the exact Delta source closure `D`; and
5. compiling `D` yields an `omega₀` tape refining the full Omega compiler it
   implements.

## Owner escalation

Escalate rather than locally redesign when `D` compilation has terrible
performance or tape size, Alpha seems too verbose, a special native accelerator
appears necessary, proof checking becomes prohibitive, or Delta compilation
requires any external older-rung semantic tool.

The exact work order lives in
[`TASKS_BOOTSTRAP.md`](../../../../TASKS_BOOTSTRAP.md).
