# Omega bootstrap checked IR v19

CKIR19 is the focused, entry-bearing flat-`TokenObservation` record-array
successor. It preserves the CKIR16/CKIR18 header and row shapes and assigns wire
major `19`, minor `0`, target `1`, flags `1`. The selected entry ID is `2`.
Major 18 is not accepted as an alias. Sums, cases, case payloads, public
constants, case arms, static byte views, record/case constructors, and opcodes
outside `1..=10` remain excluded.

This relation is a checked execution carrier for one checkpoint-shaped slice.
It does not admit the complete lexer, `TokenKind`, provider authority, a public
record ABI, or the full Omega compiler source.

## Exact selected declarations

The carrier has these three nominal records in order:

```text
Observation [copy] {
    tag: u8;
    first: u8;
    second: u8;
    third: u8;
    source: u32 in Trapping;
    start: u64 in Trapping;
    end: u64 in Trapping;
    decoded_start: u64 in Trapping;
    decoded_length: u64 in Trapping;
}

ObservationStream {
    rows: [Observation; 16384] in Trapping;
    count: u64 [0..=16384];
    last_retained: bool;
}

Main { stream: ObservationStream; }
```

Only `Observation` is `[copy]`. The private derived layout of `Observation` is
size 40, alignment 8, with field offsets `0,1,2,3,4,8,16,24,32`. The stream and
entry owner are noncopy records. The selected owner size is 655,376 bytes and
the exact backend owner/BSS ceiling is 2 MiB. A validated owner above that
ceiling selects status 252 before publication.

Type kind `8` remains ordinary unqualified CKIR `u64`: flags and reserved word
zero, with the four positional words encoding inclusive
`lower-low32, lower-high32, upper-low32, upper-high32`. Source trapping custody
on the four stored fields and read index is consumed only because their selected
uses remain scalar transport, checked Store, pure Less, or trapping IndexPlace.
This is not general arithmetic-policy erasure. The full `u32 in Trapping` field
retains kind 2 flag bit 0 and the complete unsigned `0..=0xffffffff` interval.

The machine order and signatures are exact:

1. mutable `push`, Unit result, nine scalar parameters in the Observation field
   order, three blocks;
2. shared `read_tag`, one full-width `u64` index parameter, `u8` result, three
   blocks; and
3. mutable zero-parameter `run`, `u8` result, one block and selected entry.

Machine parameter count is bounded by 16 in CKIR19; block parameters retain the
inherited ceiling of seven. All canonical call arguments are already-materialized
pure scalar literals. Structural parameters and structural call arguments are
not selected.

`run` calls `push` with receiver plus the exact source arguments
`(70,1,2,3,4,5,6,7,8)`, then calls `read_tag` with receiver plus index zero and
returns tag 70. These are two real opcode-10 calls; neither receiver boundary
may be flattened or inlined by the canonical producer. A separate handcrafted
positive uses high-half qword arguments solely to test transport; those values
are not part of canonical source lowering.

## Selected operations and custody

Opcode `4 IndexPlace` admits a kind-8 index when its base is the exact fixed
array `[Observation;16384]`. It returns the exact Observation nominal place.
The complete unsigned qword index is checked against 16,384 before any address
arithmetic. Each of the nine `push` stores independently reconstructs
`SelfPlace -> FieldPlace(rows) -> FieldPlace(count) -> Load(count) ->
IndexPlace -> FieldPlace(field) -> Store`. `read_tag` owns the tenth selected
record IndexPlace and a tag FieldPlace/Load. Every selected IndexPlace retains
its runtime check even under the true edge of the corresponding Less.

Opcode `6 Store` preserves exact scalar carrier and destination-range custody.
The selected relation stores every Observation field exactly once. Qword fields
and call parameters use exact eight-byte transport; `u32` uses four bytes and
the four `u8` fields use one byte. Opcode `5 Load` reads back the selected tag.

Opcode `8 Add` is the exact count increment, justified by the true branch of
`count < 16384`. It retains defensive qword carry and result-interval traps.
Opcode `9 Less` is pure unsigned full-qword comparison. The profile requires at
least ten record indexes, all nine distinct nested field stores, tag readback,
one selected Add, one selected Less, and exactly one call to each helper.

## Conservative x86-64 artifact

The focused backend is `omega-bootstrap-checked-ir-v19-to-elf.alp`. It accepts
only the selected major/profile and independently derives every layout, field
offset, array stride, frame slot, call scratch cell, branch, and ELF extent.

For a selected record IndexPlace it:

1. preserves the array base in `R10` and loads the full index into `RAX`;
2. loads length 16,384 into `R9`, executes `cmp rax,r9; jae trap`;
3. executes `imul rax,rax,imm32(40); jo trap`;
4. executes `add r10,rax; jb trap`; and
5. publishes the derived place only after those checks.

`FieldPlace` then adds the independently derived field offset. Add, Less,
qword load/store, call parameter installation, and destination interval checks
retain the CKIR18 conservative templates. The private call ABI remains a
scratch-cell convention, not System V or an Omega ABI.

The canonical CKIR19 fixture is 6,364 bytes with SHA-256
`eea4a3f85d3abdd452a1622671a42158ae968ec8937113892d7ea35bd32ccb66`.
Its table counts are types 12, records 3, fields 13, machines 3, machine
parameters 10, blocks 7, block parameters 0, operations 109, operands 113,
terminators 7, values 43, and places 62; all excluded tables are empty.

The canonical deterministic ELF is 8,192 bytes with SHA-256
`eb69460bad874d7cf0bbdb86efbbd878e8eafb7af0310c88f71cfb0c36b625c6`.
Its rounded writable BSS extent is 659,456 bytes and execution exits 70. These
identities witness the handcrafted CKIR fixture; source production remains a
separate lowering claim.

## Failure and evidence requirements

Malformed/profile failures select 251 with no publication. Validated public
table, byte, frame, text, or owner/BSS exhaustion selects 252 with no
publication. Required teeth include wrong major/EOF, absent entry, forbidden
type/opcode families, kind-8 policy flags, wrong record element/result/field
owner, duplicate or missing field stores, wrong call target/arity/signature,
eight/seventeen parameter profiles, high-half and exact-bound runtime indexes,
65,536/65,537 array controls, owner just over 2 MiB, and operation-table
exhaustion.

Evidence consists of independent decode/interpretation, canonical and full-path
controls, native/self Delta backend parity, exact ELF/template checks, template
mutations for IndexPlace/Add/Less/range custody, and separate independent ELF
reconstruction. No bytes are published after a 251/252 decision.
