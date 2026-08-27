# Omega bootstrap checked IR schema major 17

[`CKIR15`](OMEGA_BOOTSTRAP_CHECKED_IR_V15.md) |
[`OMGRSW9`](OMEGA_BOOTSTRAP_RESOLUTION_V9.md) |
[`OMGRFN19`](../../../refinement/delta-omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V19.md)

CKIR schema major 17 is the private platform-neutral checked-adapter
execution successor for the selected `Console` relation. It retains the two
static-attached checked adapters and the free ranked `console_write_bytes`
helper, executes their ordinary control and calls, and records ordered abstract
`Console::write_byte` events in an injected reference sink.

CKIR17 is library-shaped. It has no entry, backend, ELF, syscall, provider
installation or admission, compilation authority, or Terminal-Psi claim. A
boundary event preserves selected requirement/candidate identity; it does not
rewrite a requirement into authorized provider execution.

CKIR17 inherits CKIR15 declarations, constants, blocks, edges, runtime
shared-byte views, and opcodes 1 through 27 where selected below. It uses a new
header because four relation tables follow the inherited tables. Earlier CKIR
identities and bytes remain frozen.

## 1. Header and table order

The 116-byte little-endian header is `<8sHHHH25I>`:

```text
u8[8] magic: "OMGCKIR\0"
u16   major: 17
u16   minor: 0
u16   target: 1
u16   flags: zero
u32   entry: NO_ID
u32   exact total length
u32   twenty-one table counts in the order below
u32   reconstructed value count
u32   reconstructed place count
```

Table order and row widths are:

```text
types(24), records(20), fields(16), sums(20), cases(20),
case_payloads(16), machines(36), machine_params(20), blocks(32),
block_params(20), constants(24), constant_children(4), operations(40),
operands(4), terminators(52), case_arms(24), case_arm_args(12),
services(24), machine_reaches(12), rankings(20), boundary_targets(36)
```

Exact EOF is mandatory. CKIR17 always has flags zero and entry `NO_ID`; an
entry-bearing carrier or major-only relabel is invalid.

## 2. Selected types and receiverless machines

CKIR17 adds two kinds to the inherited type row:

| Tag | Kind | Payload 0 | Payload 1 | Range words |
| ---: | --- | --- | --- | --- |
| 9 | signed `i32` | zero | zero | signed two's-complement endpoints |
| 10 | opaque service use | service row ID | zero | both zero |

Kind 9 has flags zero. Its selected full carrier uses endpoint words
`0x80000000` and `0x7fffffff`, interpreted as signed values. Kind 10 has flags
zero and is a private opaque reference in the interpreter; the wire publishes
no address or public layout.

A kind-10 value is nonconstructible. It may occur only as a machine/block
parameter, edge argument, receiverless-call argument, or operand zero of a
`BoundaryEvent`. Constants, constant DAGs, fields, arrays, sums, loads, stores,
copies, indexing, constructors, returns, and scalar operations cannot mint or
carry one.

Machine flag bit 0 is `FREE`; bit 1 is `STATIC_ATTACHED`; they are mutually
exclusive:

- `FREE`: owner `NO_ID`, receiver access zero;
- `STATIC_ATTACHED`: provider record owner, receiver access zero; or
- inherited flags zero: record owner and shared/mutable receiver access.

The helper is free. `ConsoleNativeProvider::write` and `write_line` are
static-attached: nominal provider ownership is retained without inventing a
runtime `self`. Receiverless blocks also use access zero and cannot use
`SelfPlace`.

## 3. New relation tables

IDs are dense and all unmentioned flags are zero.

### Service row - 24 bytes (`<6I>`)

```text
id, OMGRSW9 trait ID, OMGRSW9 provider ID,
OMGRSW9 selected plan ID, selected target, flags
```

The selected row is `(0,0,0,0,1,0)` for `Console`. It is structural
provenance, never an admission receipt.

### Machine-reach row - 12 bytes (`<3I>`)

```text
id, machine ID, service ID
```

Every direct boundary event must be within its owner's reach; every ordinary
or receiverless call requires `callee reaches <= caller reaches`. The selected
helper and both adapters each have exactly one `Console` row. Missing,
duplicate, or padded reaches reject.

### Ranking row - 20 bytes (`<5I>`)

```text
id, machine ID, machine-parameter ordinal,
measure kind (1 = SliceLength), flags (bit 0 = strict recurrent descent)
```

The selected row names helper parameter `bytes`. Validation computes CFG
strongly connected components. Every cyclic edge leaving a CKIR15 nonempty
true synthetic block and entering a kind-7 parameter must carry that block's
exact `SliceTailOne` result. At least one strict recurrent descent is required.
Both guarded head/tail sites remain mandatory: initial and recurrent.

### Boundary-target row - 36 bytes (`<9I>`)

```text
id, service ID, OMGRSW9 requirement ID, OMGRSW9 plan-row ID,
OMGRSW9 candidate ID, OMGRSW9 provider ID, explicit argument type ID,
result type ID or NO_ID, binding kind (2 = CompilerIntrinsic)
```

The selected row names service 0, requirement 4, plan row 4, candidate 4,
provider 0, exact full signed-`i32`, Unit, and compiler-intrinsic binding. The
standalone checker owns local consistency; later lower-rooted evidence owns
correspondence to exact OMGCOMP3/OMGRSW9 bytes.

## 4. New operations

The inherited 40-byte operation row is unchanged.

### Opcode 28 - `ReceiverlessCall`

Immediate 0 is a free or static-attached machine; immediate 1 is zero.
Operands are only explicit arguments, with no receiver. Argument types and
result shape agree with the target. The complete machine call graph remains
finite and acyclic.

### Opcode 29 - `BoundaryEvent`

This has no result. Immediate 0 is a boundary target and immediate 1 is zero.
Operand zero is the exact kind-10 service value; remaining operands match the
target signature. The selected full-`i32` byte argument must be the result of
opcode 30 and lie in `0..=255`. Reference execution appends that mathematical
byte to the injected sink. It performs no selected-candidate dispatch or host
effect.

### Opcode 30 - `U8ToI32`

This consumes one visible exact unqualified `u8`, produces the next dense
value with exact full signed-`i32`, and has zero immediates. It is total and
payload-preserving. It corresponds only to the explicit source cast
`output as i32`; CKIR17 does not infer a call-context widening. Missing source
cast custody or a bare `u8` boundary argument rejects.

## 5. Selected library and event meaning

The selected module contains:

- free helper 0 with `(Console service, &[u8], bool)`;
- static-attached `write` 1 and `write_line` 2 with
  `(Console service, &[u8])`;
- receiverless adapter calls to helper 0 with `false` and `true`;
- two helper BoundaryEvents ordered as OMGRSW9 helper requirement calls 0/1;
- one exact ranking row and three exact reach rows.

Its exact canonical extent is 2,432 bytes. The count vector in header order is:

```text
types 6, records 1, fields 0, sums 0, cases 0, case_payloads 0,
machines 3, machine_params 7, blocks 9, block_params 14,
constants 0, constant_children 0, operations 15, operands 38,
terminators 9, case_arms 0, case_arm_args 0, services 1,
machine_reaches 3, rankings 1, boundary_targets 1, values 32, places 0
```

The deterministic handcrafted canonical image has SHA-256
`d1cfe747b0bae989f60da3ffa9c5f149579677523498d96f93e1deafdc3f75b7`.
The digest is fixture identity, not package, provider, or compilation authority.

Reference invocation supplies an opaque service object plus runtime view. The
required observations are:

```text
write([])          -> []
write([70])        -> [70]
write_line([70,71])-> [70,71,10]
write_line([])     -> [10]
```

The event trace is bounded to 65,536 bytes, dynamic block entry to 262,144,
and active receiverless frames to 64. Exhaustion selects 252 before result
publication.

## 6. Resources, statuses, and exclusions

Inherited CKIR15 ceilings remain. New ceilings are 128 services, 4,096
machine reaches, 128 rankings, and 4,096 boundary targets. The complete byte
ceiling is 2,654,288. Resource excess selects 252. Malformed identity, EOF,
receiver class, service custody, reach closure, ranking/SCC descent, signature,
event, explicit widen, call, or selected profile selects 251. Neither status
publishes validation/event bytes.

Focused evidence is `../gates/delta-checked-ir-v17-reference.sh`, backed by
`checked_ir_v17_reference.py` and the handcrafted fixture/mutation corpus.

CKIR17 excludes a source producer, source-root invocation, provider
installation, native provider substitution, boundary call plan, syscall,
backend, ELF, artifact reconstruction, provider/package admission,
compilation authority, and final `Omega_self` admission. Those are explicit
nonclaims, not unfinished authority hidden behind the event sink.
