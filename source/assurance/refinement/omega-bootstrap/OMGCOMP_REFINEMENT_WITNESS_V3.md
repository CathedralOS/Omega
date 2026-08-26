# OMGCOMP lower-rooted refinement witness, version 3

This contract versions the private refinement frame for the first exact-root,
attached-machine-call `CKIR2` tranche. It inherits the complete `OMGCOMP` and
`OMGRSW1` row schemas, ordering, source custody, and ceilings from
[`OMGCOMP_REFINEMENT_WITNESS.md`](OMGCOMP_REFINEMENT_WITNESS.md). The witness
schema remains `OMGRSW1`; the frame changes because the claimed checked IR and
artifact relation change.

The canonical source fixture is produced by
[`role3_resolution_fixture.py`](../../../../bootstrap/omega-bootstrap/gates/role3_resolution_fixture.py).
It uses two source files in one logical module, an exact selected root, a
three-machine finite call chain, and an unreachable decoy. This does not settle
private access between distinct modules, cross-package machine calls, general
member receivers, or recursion.

## Version-3 refinement frame

```text
offset  width  field
0       8      magic: ASCII "OMGRFN3\0"
8       u32    version: 3
12      u32    flags: 0 library, 1 entry-bearing compilation
16      u32    exact OMGCOMP byte length
20      u32    exact OMGRSW1 byte length
24      u32    exact CKIR2 byte length
28      u32    exact ELF byte length
32      u32    claimed full result
36      u32    claimed exit projection
40      ...    OMGCOMP || OMGRSW1 || CKIR2 || ELF || exact EOF
```

The library/entry rules and component ceilings remain those of `OMGRFN2`:
267,280 OMGCOMP bytes, 524,288 witness bytes, 2,260,040 CKIR bytes, and
1,052,672 ELF bytes. The simultaneous maximum remains 4,104,320 bytes. Checked
addition precedes every offset. Excess is 252; malformed fields, relations,
arithmetic, and EOF are 251. Nothing is published on rejection.

The frame contains no digest, accepted-lock receipt, or compiler authority.
The independently accepted compilation-authority join remains external.

## Versioned semantic obligations

Every unchanged `OMGRFN2` obligation remains in force. The following replace
the CKIR1-specific parts:

- CKIR has schema major 2, flag bit 0 set for an entry, and the exact selected
  machine ID reconstructed from the OMGCOMP root and OMGRSW1 declaration;
- every authored `self.name(...)` call consumes exactly one role-3 binding with
  the same source/token span and resolved machine declaration;
- lowering emits receiver first, then explicit arguments left-to-right, then
  one opcode-10 row with exact callee, arity, receiver access, argument types,
  and Unit/scalar result shape;
- every role-3 row is consumed once and the complete machine call graph,
  including unreachable machines, is acyclic;
- source-only and CKIR-only evaluators execute calls with distinct frames over
  one shared owner memory and independently reproduce the claimed full result;
  and
- ELF reconstruction emits only the selected reachable closure and recomputes
  per-machine frames, call staging, the private `rdi`/`rsi`/`eax` ABI, rel32
  call targets, maximum live stack, and exact image bytes.

The checked-IR rules and backend resource ceilings are normative in
[`OMEGA_BOOTSTRAP_CHECKED_IR_V2.md`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V2.md).
The physically artifact-free source evaluator supports at most 16 active
machine frames; a 17th active frame reports 252. The CKIR2 evaluator supports at
most 64 active machine frames; a 65th reports 252. These are checker-evidence
storage ceilings, not source-profile recursion admissions. The complete source
and CKIR call graphs must still be finite and acyclic, including unreachable
machines. Evidence exhaustion is never permission to call a malformed or cyclic
artifact valid.

## Independent checker split

No checker imports a producer conclusion. Five persisted-Beta responsibilities
consume the same exact `OMGRFN3` bytes:

1. frame and complete OMGCOMP/source custody;
2. independent source-to-OMGRSW1 resolution, including exact role-3 rows;
3. OMGRSW1-to-CKIR2 declarations, layout, types, explicit root, and table rows;
4. resolved source bodies to CKIR2 operations/calls/terminators plus a
   physically artifact-free source result; and
5. complete CKIR2/result validation and CKIR2-to-ELF reconstruction.

Layer-local controls cover frame/version cross-rejection, root drift, binding
target/span/order/consumption, callee/signature/receiver/argument/result drift,
direct and unreachable call cycles, table and live-stack exhaustion, result
cross-pairs, rel32 targets, ABI staging, reachable closure, and exact ELF bytes.
`OMGRFN2` remains frozen; none of its checkers may be relabeled as CKIR2
evidence merely because the outer component offsets are unchanged.

The conjunction establishes the selected finite, acyclic, returning
source-to-artifact relation. It does not admit recursion or a source family to
final `Ωself`, and it does not grant compilation authority.
