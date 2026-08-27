# OMGCOMP refinement witness v22

Status: private lower-rooted closure for the focused flat guarded
`Observation` record-array capability. The reference/Beta gate and the
separate actual-producer native/self same-frame gate pass. Product, package,
accepted-lock, provider, and compilation authority remain separate.

[`OMGRSWB11`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V11.md) |
[`OMGLOWK20`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_LOWERING_V20.md) |
[`CKIR19`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V19.md)

OMGRFN22 is deliberately narrower than `TokenStream`. It owns a flat copyable
`Observation` record with four `u8`, one explicitly trapping `u32`, and four
explicitly trapping `u64` leaves; a noncopy stream record with one trapping
fixed `[Observation; 16,384]`, one `u64 [0..=16,384]` count, and one Boolean;
and a noncopy root record containing that stream. It proves guarded record
IndexPlace followed by scalar FieldPlace/Store, guarded FieldPlace/Load
readback, and the Exact retained-count increment. Nested records, sums,
structural values or parameters, slices, computed/effectful indexes, indirect
calls, constructors, and unrelated arithmetic are outside this cut.

The exact 40-byte little-endian outer header uses magic `OMGRFNM\0`, major 22,
flags 1, four component lengths, result 70, and exit projection 70. Components
are exact OMGCOMP1, OMGRSWB major 11, CKIR major 19, and conservative Linux
x86-64 ELF, in that order. Every component is nonempty; component ceilings,
extents, the inherited whole-frame ceiling, and EOF are exact.

The canonical component identities are:

- OMGCOMP1: 1,884 bytes, SHA-256
  `6782c9e5d3f282cb20d05f5b182ebc58132f9c653c613c3b6b3b28744aae797d`;
- OMGRSWB11: 2,172 bytes, SHA-256
  `00727b9c80aec71054a20dbc7afe80d8b587d377ebf22e04c45e0c5a164ebe05`;
- CKIR19: 6,364 bytes, SHA-256
  `eea4a3f85d3abdd452a1622671a42158ae968ec8937113892d7ea35bd32ccb66`;
  and
- ELF: 8,192 bytes, SHA-256
  `eb69460bad874d7cf0bbdb86efbbd878e8eafb7af0310c88f71cfb0c36b625c6`.

These identify the canonical evidence, not a digest allowlist. OMGRSWB11 admits
the resolver's exact rename, independent declaration reorder, observation
field reorder, and comment/inert-field family. Every accepted producer variant
lowers to the same normalized CKIR19 bytes.

## Admitted relation and erased facts

Only `Observation` is authored `[copy]`; OMGRSWB11 must preserve that bit and
must preserve noncopy `ObservationStream` and root. The normalized field owner,
ordinal, source type, and selected source-name spans form one complete
partition. The source `u32 in Trapping` and four source `u64 in Trapping`
leaves remain authored-policy facts in OMGRSWB11. Their CKIR scalar types and
operations own runtime checking; the assurance relation never fabricates a
qualifier from CKIR use.

The writer first evaluates the direct count leaf against literal 16,384. Only
the true block forms `rows[count]`, then the nine scalar FieldPlace/Store paths,
then evaluates the authored Exact expression `count + 1`. The true-edge fact
is `count <= 16,383`: record indexing is in bounds, unsigned addition cannot
carry, and the result remains in `0..=16,384`. The CKIR Add retains defensive
carrier and result-interval checks, but neither may fire for admitted source.
The false block performs no record access or increment.

The reader first evaluates its exact trapping-u64 parameter against the direct
count leaf. Only the true block forms `rows[index].tag` and loads the `u8` leaf.
Because `index < count <= 16,384`, IndexPlace is in bounds. Index `N-1` is an
admitted runtime boundary; index `N` and a nonzero-high-half index reach the
defensive bounds trap. The root passes the exact pure scalar literals
`70,1,2,3,4,5,6,7,8`, then reads index zero and returns 70. A separate CKIR
positive owns high-half call transport and is intentionally rejected as a
source/CKIR cross-pair for the canonical OMGRFN22 frame.

CKIR19 independently derives `Observation` layout size 40, alignment 8, field
offsets 0, 1, 2, 3, 4, 8, 16, 24, and 32. The stream/root private layout is
655,376 bytes and the conservative BSS is 659,456 bytes. The selected entry
owner must not exceed 2 MiB. An immediately-below-cap noncanonical owner is
semantic malformed 251 for this fixed relation; the adjacent 52,429-element
owner crossing 2 MiB is resource exhaustion 252.

## Modular responsibility owners

- R1 owns OMGRFNM/22 identity, flags, exact component identities and extents,
  inherited ceilings, complete OMGCOMP1 custody, result/exit 70, and EOF.
- R2 independently reconstructs OMGCOMP1 to OMGRSWB11. It owns the ten dense
  tables, authored spans, record copy policy, record/field owner and ordinal,
  scalar policies and full-width endpoints, machine/parameter/block/call
  identity, nine direct store paths and call arguments, both direct guards,
  Exact leaf-plus-literal increment, readback path, exclusions, selected root,
  and source/witness cross-pair rejection. It reads no CKIR or ELF.
- R3 reconstructs the complete producer-facing CKIR19 structure: exact
  three-record layouts, machine and parameter access, ten record IndexPlace
  rows, the nested FieldPlace partition, nine scalar Stores, one scalar Load,
  two u64 Less guards, one u64 Add, two direct calls, values/places,
  operation/operand/terminator visibility, true-block custody, all excluded
  tables/opcodes, and CKIR-local malformed/resource status.
- R4-lowering joins exact OMGRSWB11 identities to CKIR record, field, carrier,
  parameter, guard, path, store/load, literal, call, result, and policy-erasure
  identities. It owns the two true-edge range arguments above. R4-source-result
  observes result 70 from the admitted source/witness relation without CKIR or
  ELF.
- R5-structure invokes the frozen independent CKIR19 meaning. R5-result owns
  CKIR execution, result 70, and the boundary observations. R5-ELF independently
  reconstructs every artifact byte without invoking the production backend.
  It imports the earlier modular qword/scalar owners and adds only record-array
  indexing: unsigned qword `index < N`, signed checked multiplication by the
  derived 40-byte stride, checked address addition, and the inherited exact
  field offsets, scalar widths, calls, frames, ranges, and ELF orchestration.

Acceptance is the conjunction of these owners over one immutable frame. No
owner imports another owner's verdict. Complete source reconstruction, CKIR
structure, abstract execution, and exact artifact bytes remain distinct
obligations.

## Controls, resources, and lower-root evidence

Responsibility-local controls cover outer/component version, flags, extents,
EOF, result, and ceilings; source/witness cross-pairs; record copy,
owner/ordinal/type, authored scalar policy and endpoints, guard and Exact
increment drift; CKIR record layout/stride, FieldPlace path, IndexPlace,
Store/Load access and widths, call literals, and CKIR cross-pairs; `N-1`,
index-`N`, and high-half runtime behavior; below/above-2-MiB owner boundaries;
operation-table exhaustion; and ELF cross-pairs plus stride, overflow branch,
address-add, field-offset, truncation, and arbitrary-byte mutations. Every 251
or 252 path publishes no bytes.

The reference gate runs eight complete modular Python owners and 21 smaller
representative persisted-Beta projections. Each Beta projection compiles under
both persisted `bc` and self-produced `bc`, requires byte-identical assembly,
accepts its assigned canonical frame, rejects one responsibility-local byte,
publishes nothing on rejection, and remains below the explicit 230-KiB target.
Observed tapes range from 62,540 to 218,038 bytes; the complete list is emitted
by `omgrfn22-beta-join.sh`. These finite projections establish representative
persisted lower-root lineage. They do not replace the complete Python owners
or constitute producer evidence.

The separate actual-producer same-frame gate compiles the focused OMGRSWB11
resolver, OMGLOWK20 lowerer, and CKIR19 backend through both native Delta and
the self-produced lowermachine route. It checks every accepted source variant,
exact native/self OMGRSWB11, CKIR19, and ELF identity, actual source/witness,
CKIR, and ELF cross-pairs, one no-publication resource tooth per producer
phase, an artifact byte tooth, and the unmodified actual canonical frame
through every R1-R5 owner. No handcrafted component is substituted for that
production obligation.
