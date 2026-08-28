# OMGCOMP refinement witness v20

Status: private platform-neutral lower-rooted assurance for the exact
OMGCOMP3 + OMGRSW9 + CKIR17 checked `Console` adapter execution relation.

OMGRFN20 contains no ELF, native effect, provider installation or admission,
accepted-lock evidence, package authority, compilation authority, or
Terminal-Psi `ProviderExecution`.

## Carrier

The 36-byte little-endian header is `<8s4H5I>`: magic `OMGRFNK\0`, major 20,
minor zero, flags zero, header size 36, exact total length, exact OMGCOMP3
length, exact OMGRSW9 length, exact CKIR17 length, and one zero reserved word.
The three exact nonempty components follow in that order and exact EOF follows
CKIR17. Component ceilings are 267,280, 524,288, and 2,654,288 bytes; the
whole-frame ceiling is 3,445,892 bytes.

CKIR17 is a flags-zero, `NO_ID`-entry library. Its injected reference API may
invoke static-attached adapters 1 or 2 with an opaque Console token and runtime
bytes. It records abstract `Console::write_byte` events; it never dispatches a
candidate or performs a host effect.

## Selected relation

OMGRSW9 plan row 1 selects checked candidate 1,
`ConsoleNativeProvider::write`, for requirement 1. The adapter calls free
helper 0, `console_write_bytes`, with `(Console, &[u8], false)`. The helper
retains both guarded head/tail sites, exact `Slice::Length` strict recurrent
descent, and `reaches Console`. Its two authored `output as i32` casts lower to
opcode 30 `U8ToI32`; no call-context inference may invent those widenings.
Each widened byte feeds opcode 29 `BoundaryEvent` for exact requirement 4 and
boundary target `(service 0, requirement 4, plan row 4, candidate 4, provider
0, full i32, Unit, CompilerIntrinsic)`. The event remains requirement-targeted
and does not authorize candidate 4.

The four mandatory observations are `write([]) -> []`, `write([70]) -> [70]`,
`write_line([70,71]) -> [70,71,10]`, and `write_line([]) -> [10]`.

## Responsibilities

- R1 owns OMGRFN20 identity, flags, extents, component ceilings, complete
  OMGCOMP3 structural custody, exact component identities, and EOF.
- R2 owns exact source and OMGRSW9 custody: build role, complete selected plan,
  provider, helper, adapters, reach/ranking source clauses, ordinary calls,
  exact explicit-cast requirement calls, and requirement/candidate separation.
- R3 owns complete CKIR17 validation and EOF: services, free/static-attached
  machine classes, reach closure, strict ranking/SCC descent, boundary target,
  inherited recurrent edges, opcodes 28/29/30, values, and resources.
- R4 owns the source/witness/CKIR join: helper and adapter identities, explicit
  cast-to-widen correspondence, receiverless calls, service custody, plan-row
  and requirement-targeted events, reach, ranking, and cross-pair rejection.
- R5 independently evaluates the immutable CKIR17 bytes through the injected
  adapter API and owns exact bounded abstract event observations.

Acceptance is conjunctive over one immutable frame. Owners may share raw
decoders, but no owner imports another owner's conclusion. Malformed framing,
identity, cross-pair, source span, service, reach, ranking, call, widen, event,
trace, table, or EOF state selects 251 without publication. Declared component,
table, step, frame, trace, or whole-frame exhaustion selects 252 without
publication.

This evidence proves checked adapter execution only. Provider admission later
must separately consume selected-plan facts, resolve the checked-adapter
catalog, admit exact terminal IDs, install provider roots, and supply admitted
`ProviderExecution` values.

## Evidence state

`omgrfn20-reference.sh` runs the modular Python R1--R5 conjunction and five
representative persisted-Beta projections over the same immutable frame. Each
projection has a responsibility-local rejection tooth, is assembled through
both persisted Beta compiler generations with identical assembly, and remains
below the 262,140-byte tape ceiling. The Python corpus additionally owns exact
EOF, component/table/frame resources, cross-pair, source-cast, selected-plan,
service/ranking/boundary, and structurally valid trace mutations.

The exact components are the 3,335-byte OMGCOMP3 envelope, 2,304-byte OMGRSW9
witness, and independently checked 2,432-byte CKIR17 library whose SHA-256 is
`d1cfe747b0bae989f60da3ffa9c5f149579677523498d96f93e1deafdc3f75b7`.
The former producer-dependent same-frame wrapper is retired until the
canonical Delta compiler is published. Fixture construction remains reference
evidence only and makes no producer-publication claim; the R1--R5 owners remain
responsibility-local.
