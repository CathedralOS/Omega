# OMGCOMP lower-rooted refinement witness, version 8

[`OMGRFN7`](OMGCOMP_REFINEMENT_WITNESS_V7.md) |
[`OMGRSW1`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[`CKIR6`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V6.md) |
[`CKIR6 backend`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V6_BACKEND.md)

`OMGRFN8` is the private lower-rooted carrier for bool-only prefix logical
negation. It preserves the five independent responsibility owners and adds
only the source-to-CKIR6 and CKIR6-to-artifact relations required for opcode 15
`LogicalNot`. Logical negation adds no resolution identity, so this carrier
accepts the exact least canonical OMGRSW1, OMGRSW2, or OMGRSW3 selected by its
source. Every earlier OMGRFN, OMGRSW, OMGLOW, and CKIR identity remains frozen.

## Version-8 frame

The outer 40-byte little-endian layout is:

```text
offset  width  field
0       8      magic: ASCII "OMGRFN8\0"
8       u32    version: 8
12      u32    flags: 0 library, 1 entry-bearing compilation
16      u32    exact OMGCOMP byte length
20      u32    exact selected OMGRSW1/2/3 byte length
24      u32    exact CKIR6 byte length
28      u32    exact ELF byte length
32      u32    claimed full result
36      u32    claimed exit projection
40      ...    OMGCOMP || selected OMGRSW1/2/3 || CKIR6 || ELF || exact EOF
```

There is deliberately no outer witness selector. The selected witness's exact
magic and schema major self-identify it; the resolution owner validates that
identity and reconstructs whether it is the least canonical relation for the
exact source. Adding a second producer-supplied selector would duplicate that
identity without adding evidence. The frame likewise carries no producer-
selected subject, observation profile, semantics version, target capsule,
bridge, admission, digest, or authority field. Verifiers reconstruct those
obligations from the exact components and pinned contracts.

The component ceilings remain 267,280 OMGCOMP bytes, 524,288 selected-witness
bytes, 2,522,192 CKIR6 bytes, and 1,183,744 ELF bytes. Consequently the exact
whole-frame ceiling remains:

```text
40 + 267,280 + 524,288 + 2,522,192 + 1,183,744 = 4,497,544 bytes
```

Checked addition precedes every offset and complete-frame calculation. A
validated component extent above its ceiling is status 252. Malformed framing,
identity, version, flags, component relation, arithmetic, truncation, or
trailing bytes is status 251. Neither status publishes output, and a selected
252 is not downgraded by later inspection.

An entry frame has flags 1, a nonempty ELF, one full unsigned 32-bit result,
and exit projection `result & 255`. A library frame has flags 0, an empty ELF,
and `0xffffffff` in both result fields. OMGCOMP, the selected witness, and CKIR6
are nonempty in both modes. No other flag value or result relation is valid.

An untrusted packer may create these exact bytes. It grants no proposition;
least-version selection, component correspondence, meaning, and artifact
reconstruction remain independently checked.

## Exact identities and least resolution

OMGRFN8 pairs only with CKIR schema major 6, minor 0, target 1
(`linux_x86_64`). CKIR6 requires at least one opcode-15 row. Its source relation
requires at least one admitted authored `!`; changing only an old CKIR or frame
major cannot satisfy either requirement.

The resolution owner reconstructs this least-version rule directly from exact
OMGCOMP source:

| Exact source closure | Required witness |
| --- | --- |
| no pure sum and no direct field-receiver call | byte-identical OMGRSW1 |
| at least one direct field-receiver call and no pure sum | OMGRSW2 |
| at least one pure sum, with or without direct field-receiver calls | OMGRSW3 |

Logical negation does not affect this table and creates no OMGRSW4. A
well-formed but nonleast witness rejects. OMGRFN8/CKIR6, OMGRFN8/OMGRSW1-3, and
each source/witness relation pair exactly; all earlier/newer or nonleast cross-
pairs reject at the responsibility that owns the corresponding join.

## Subject-qualified refinement claim

For an entry frame, the terminal claim is not a profile-free assertion that
the artifact is "verified." It is the following bounded, subject-qualified
operational-refinement judgment:

- the source subject is the exact OMGCOMP bytes, exact selected root, and their
  interpretation under the inherited source contracts and the lowering
  relation pinned by CKIR6 sections 1 and 2;
- the artifact subject is the exact ELF bytes interpreted under CKIR6 target 1,
  the inherited Linux x86-64 System V target relation, and the pinned CKIR6
  conservative instruction templates;
- the exact least OMGRSW1/2/3 and exact CKIR6 bytes are identity-bearing bridge
  subjects, not producer verdicts;
- the verifier-reconstructed observation profile for this bounded closed-entry
  tranche contains the selected computation's full unsigned 32-bit result and
  its Linux process exit projection `result & 255`; the full-result observation
  may not be weakened to exit status alone; and
- the checked bridge graph is R4's exact source-to-CKIR6 lowering and source
  meaning joined on identical CKIR6 bytes to R5's CKIR6 meaning and exact
  formal-artifact reconstruction, with R2 and R3 supplying the independently
  checked resolution and declaration joins.

Exact ELF identity is part of the artifact subject and reconstruction
obligation; it is not a producer-selected observation profile. The versioned
contracts above determine the source semantics, target semantics, carrier
identities, target capsule, and bridge directions. Any report of acceptance
must name these subjects, versions, profile, bridge graph, and any admissions.
No intended-mathematical-model or global-theory subject participates in this
operational Boolean tranche, so it introduces no model/theory bridge.

This reusable artifact claim stops at the formal target. Native execution is
regression evidence, not proof that physical silicon implements that target.
A deployment that makes that connection must separately disclose the physical-
target realization admission and may not fold it into the OMGRFN8 artifact
claim.

## Responsibility-local propositions

All owners consume the same immutable OMGRFN8 bytes. A responsibility may be
implemented by multiple bounded executables, but no owner imports another
owner's process-local state or a producer conclusion.

1. **Frame and source custody** validates exact OMGRFN8 framing, bounds, mode,
   result-field shape, and EOF and independently reconstructs complete OMGCOMP
   custody. Witness, CKIR6, ELF, and claimed result bytes remain opaque.
2. **Resolution** independently reconstructs the exact least canonical
   OMGRSW1, OMGRSW2, or OMGRSW3 from exact source. It retains every frozen
   source-custody, visibility, binding, direct-field-receiver, and pure-sum
   relation belonging to the selected version. It has no CKIR or ELF access;
   `!` adds no resolution fact.
3. **Declarations and intrinsic CKIR structure** validates the exact selected
   witness and CKIR6 identities and joins all inherited declarations, types,
   layouts, selected root, tables, and intrinsic row envelopes. It validates
   opcode 15's structural envelope and the CKIR6 requirement that one exists,
   but does not claim correspondence to an authored token, compute source-body
   meaning, or reconstruct ELF.
4. **Source lowering and source meaning** independently reparses exact bodies.
   It proves bool-only prefix typing and precedence, one evaluation and one
   opcode-15 operation per authored `!`, including literals and adjacent
   negations, and no constant folding. Its artifact-free evaluator proves both
   truth rows and reconstructs the exact full source result; its lowering
   independently reconstructs exact CKIR6. It has no ELF access.
5. **CKIR meaning and artifact reconstruction** remains source-body and
   witness-identity opaque. It validates complete CKIR6, evaluates opcode 15 as
   `1 - operand` over canonical Boolean zero/one, derives the full result, and
   reconstructs the exact Linux x86-64 ELF, including the pinned
   load/XOR-one/store instruction sequence.

The composition accepts only if every responsibility accepts its own
proposition over the identical component bytes and the independently derived
results agree. Splitting owners into named functions within one shared
producer verdict is not this conjunction.

## Required same-frame evidence

The primary immutable positive frame must extend the complete OMGRFN7 payload-
sum carrier and therefore select OMGRSW3. It must retain a nonzero case tag and
bound payload, payload-free and payload-bearing cases, one-to-four recursively
copyable payload fields, nested aggregate payloads, runtime construction,
Copy, a structural Call argument, parameter and nonzero-offset `self`-field
dispatch, selected payload binding, and exact result 70.

The selected reachable computation and emitted ELF must additionally depend on
logical negation. The same carrier exercises reachable `true -> false` and
`false -> true` observations plus an adjacent `!!` form, emits one opcode-15
row for each authored `!`, and contains the exact reachable load/XOR-one/store
templates. An unreachable extra machine containing `!` is not sufficient.
Every native and Delta-self-built R1/R2/R3/R4/R5 conjunct runs over these same
bytes. Compact OMGRSW1 and OMGRSW2 logical-negation positives remain focused
controls proving generic least-resolution support; they need not replace the
full OMGRSW3 composition carrier.

Phase-local mutations and valid-but-mismatched source/witness, witness/CKIR6,
CKIR6/ELF, and result pairs prove the ownership matrix. Required version teeth
include OMGRFN7/8, CKIR5/6, and least/nonleast OMGRSW1/2/3 cross-pairs. Exact-
subject and profile teeth include different source or artifact bytes that
retain the same result, two full results differing by 256 that retain the same
exit projection, target/header/machine mutations, and appended producer
"profile" or trailing bytes. Native execution cannot stand in for formal
artifact reconstruction. Frozen OMGRFN5 through OMGRFN7 positives and
separation controls remain live.

The executable lower-rooted evidence is deliberately materialized without
invoking sibling gates. `omgrfn8-materialize-r1-r2.py` and
`omgrfn8-materialize-r3-r5.py` produce the eight bounded Beta responsibility
programs as pure source transformations. The former producer-dependent
same-frame wrapper is retired until the canonical Delta compiler is published;
the responsibility-local owners remain independent of that producer.

Logical negation consumes the inherited expression-depth, operation, operand,
value, and four-byte scalar-slot resources and introduces no new ceiling. The
evidence retains exact/adjacent depth 8/9, operation, operand, value, frame,
text, CKIR, ELF, evaluator, and 251/252-with-empty-output teeth.

## Non-expansion

OMGRFN8 proves only the bounded bool-only logical-negation relation specified
by CKIR6, composed with whichever already-frozen least OMGRSW1/2/3 relation the
exact source requires. It does not add general unary operators, integer
truthiness, bitwise complement, short-circuiting, constant folding, a new
resolution identity, or a new proof-kernel capability. It grants no package or
accepted-lock authority, compilation authority, public ABI, proof authority,
Terminal-Psi dependency, physical-target guarantee, final `Ωself` admission,
or additional build rung.
