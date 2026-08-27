# OMGCOMP lower-rooted refinement witness, version 6

This contract versions the private refinement carrier for the direct nominal
field-receiver call tranche. It inherits the complete `OMGCOMP`, `CKIR4`, ELF,
result, resource, and status rules from
[`OMGRFN5`](OMGCOMP_REFINEMENT_WITNESS_V5.md). Its only component change is the
canonical [`OMGRSW2`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V2.md)
resolution relation and the corresponding
[`OMGLOW5`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLVED_TO_CKIR4_V2.md)
source-to-CKIR4 relation. It does not widen CKIR4.

## Version-6 refinement frame

`OMGRFN6` uses the same 40-byte little-endian header and component ceilings as
OMGRFN5:

```text
offset  width  field
0       8      magic: ASCII "OMGRFN6\0"
8       u32    version: 6
12      u32    flags: 0 library, 1 entry-bearing compilation
16      u32    exact OMGCOMP byte length
20      u32    exact OMGRSW2 byte length
24      u32    exact CKIR4 byte length
28      u32    exact ELF byte length
32      u32    claimed full result
36      u32    claimed exit projection
40      ...    OMGCOMP || OMGRSW2 || CKIR4 || ELF || exact EOF
```

The ceilings remain 267,280 OMGCOMP bytes, 524,288 OMGRSW2 bytes, 2,522,192
CKIR4 bytes, 1,183,744 ELF bytes, and 4,497,544 bytes for the complete frame.
The inherited checked-addition, entry/library, 0/251/252, publication, and
exact-EOF rules remain normative.

Carrier identities are exact. OMGRFN5 composes with OMGRSW1; OMGRFN6 composes
with OMGRSW2. The resolution owner rejects either cross-pair. The frame/source
custody and CKIR/result/ELF owners retain phase-local opacity: they validate the
outer OMGRFN version they own but do not inspect or duplicate the resolution
witness's inner identity. A changed opaque witness must not alter their
verdict. OMGRFN5 remains frozen.

## Responsibility-local change

The five-responsibility split from OMGRFN5 remains in force. Version 6 changes
only the propositions that actually depend on the new source relation:

1. **Frame and source custody** accepts the exact OMGRFN6 envelope and
   independently reconstructs OMGCOMP, while treating OMGRSW2, CKIR4, and ELF
   as opaque component bytes.
2. **Resolution** reconstructs canonical OMGRSW2 directly from exact source.
   In addition to inherited `self.machine(...)`, it recognizes only exact
   `self.field.machine(...)`, resolves `field` on the caller's nominal owner,
   requires a nominal field type and exact same-package/logical-module callee,
   emits the ordinary role-3 callee binding, and requires at least one such
   direct field call for version 6. It independently rejects absent, scalar,
   indexed, computed, parameter, parenthesized, chained, wrong-owner,
   cross-module, and cross-package receiver forms.
3. **Declarations and intrinsic CKIR structure** owns the exact
   OMGRSW2-to-CKIR4 declaration/table join, including the version/witness pair.
   CKIR4's schema and intrinsic structure are unchanged.
4. **Source lowering and source meaning** reconstructs the exact
   `SelfPlace -> FieldPlace -> Call` sequence for each direct field receiver.
   Its physically artifact-free evaluator carries the current receiver base at
   every call depth: `SelfPlace` denotes that base, `FieldPlace` adds the
   independently derived nominal field offset, and a call passes operand zero's
   place address as the callee receiver. No producer-supplied field path,
   offset, or receiver base is evidence.
5. **CKIR meaning and artifact reconstruction** retains the complete CKIR4
   result and exact ELF propositions. It accepts an OMGRFN6 outer envelope but
   remains source-body and witness-identity opaque.

The same-frame composition gate invokes every responsibility over the same
immutable carrier. It includes phase-local mutations and distinct
source/witness, witness/CKIR4, CKIR4/ELF, and result cross-pairs; it does not
replace responsibility-local reconstruction with a shared producer verdict.

## Required boundary evidence

The focused positive must place the receiver record at a nonzero enclosing
field offset and exercise both mutable Unit-returning and shared scalar-returning
attached calls, producing exact result 70. The exact product `SourceUnit`
source plus its same-logical-module field harness supplies the product-shaped
carrier; the compact direct-field carrier supplies tractable Rust-free meaning
and mutation teeth. Native and Delta-self-built production must agree exactly.

Negatives isolate scalar or unknown fields, wrong nominal owners, shared-to-
mutable calls, receiver-shape variants, call cycles, role-3 row mutations,
OMGRFN5/6 and OMGRSW1/2 cross-pairs, receiver-base/address mutations, and
source/witness, witness/CKIR4, CKIR4/ELF, and result mismatches. Existing CKIR4
constructor, call, frame, stack, instruction, and 0/251/252 resource teeth
remain applicable without relabeling them as new source capability.

## Non-expansion

This version admits only the direct same-module nominal field receiver already
specified by OMGRSW2 and OMGLOW5. It does not decide private access across
logical modules, effectful/trapping aggregate-field evaluation order, final
`Ωself` retention, compilation authority, Terminal-Psi use, or another build
rung. The carrier is bridge-private evidence transport.
