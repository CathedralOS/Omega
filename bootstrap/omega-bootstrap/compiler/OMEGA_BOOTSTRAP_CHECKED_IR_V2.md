# Omega bootstrap checked IR schema major 2

CKIR schema major 2 is the private, versioned successor used by the first
explicit-root and attached-machine-call bridge tranche. It is not an Omega ABI,
does not admit recursion, and does not widen the final `Ωself` source profile.

Except for the overrides below, every byte-level row definition, dense-ID rule,
type/layout rule, operation rule for opcodes 1 through 9, terminator rule,
canonical source order, status, and publication rule in
[`OMEGA_BOOTSTRAP_CHECKED_IR.md`](OMEGA_BOOTSTRAP_CHECKED_IR.md) remains
normative.

## Version and framing

The eight-byte CKIR magic remains exactly `OMGCKIR\0`. The schema major is `2`,
the schema minor is `0`, and the target remains `1` (`linux_x86_64`). Header and
table strides are unchanged. A schema-major-1 consumer must reject schema 2,
and a schema-major-2 consumer must reject schema 1.

The resolved-source input uses the separate `OMGLOW2\0` frame, major 2, minor
0. Its 32-byte layout and bounded `OMGCOMP || OMGRSW1` components are otherwise
identical to OMGLOW1. This new identity prevents accidental selection of the
CKIR1 lowerer while preserving OMGLOW1 bytes and behavior.

## Exact selected root

Flag bit 0 is set and the CKIR entry machine ID equals the exact selected
machine ID carried by OMGRSW1. The lowerer validates that selection against the
OMGCOMP package, source, module, owner, machine name, and entry signature before
publication.

Schema 2 has no global candidate-cardinality rule. Other zero-parameter scalar
machines are valid and do not compete with the selected root. The selected
entry still has zero explicit parameters, a scalar result, a zero-parameter
entry block, and a recursively zero-establishable owner.

## Opcode 10: attached `Call`

The existing 40-byte operation row encodes `Call` with opcode 10:

- immediate 0 is the dense callee machine ID and immediate 1 is zero;
- operand 0 is a place of the callee owner's exact nominal type;
- remaining operands are values for the callee's explicit parameters in
  parameter order;
- operand count is therefore `1 + callee_parameter_count`, from 1 through 8;
- a mutable callee requires a mutable receiver place; a shared callee accepts a
  shared or mutable receiver;
- each argument has the callee parameter's exact type after the ordinary
  literal/range materialization rules;
- a Unit callee has result kind 0, `NO_ID` result ID/type, and produces no
  value; a scalar callee has result kind 1 and produces one value of its exact
  result type; structural results remain unsupported; and
- operation owner machine/block, dense result IDs, visibility, and operand
  spans obey the existing rules.

Every authored attached call consumes exactly one OMGRSW1 role-3 binding whose
source and exact callee-token span match the call. That binding targets the
same machine declaration encoded in immediate 0. Lowering joins this identity;
it must not repeat name resolution. Every role-3 row is consumed exactly once.

For canonical source lowering, the receiver expression is lowered first.
Explicit arguments are then lowered and materialized left-to-right. The Call
row is emitted after its receiver and all arguments. Nested calls follow the
same rule at expression depth at most eight.

## Canonical private call ABI and templates

Opcode 10 uses a bridge-private, closed-image calling convention. It is not the
System V source ABI and is not an FFI promise. `rdi` carries the callee receiver
address, `rsi` points at caller-owned eight-byte argument cells, and `eax`
carries a scalar result. Structural arguments are immutable addresses. All
other state is private to the two machine frames; no call argument or result is
published in ELF metadata.

### Reachable-machine frames and shared scratch

The backend reconstructs the closure reachable from the exact selected entry
through opcode-10 edges. For each reachable machine in machine-ID order it
assigns one frame independently:

1. the incoming receiver address occupies the eight-byte slot at `[rbp-8]`;
2. values owned by the machine are visited in global value-ID order, with a
   scalar aligned to four bytes and occupying four bytes and a structural value
   aligned to eight bytes and occupying one eight-byte address slot;
3. places owned by the machine are visited in global place-ID order, aligned to
   eight bytes, and occupy one eight-byte address slot each;
4. the shared scratch-cell count is the maximum of both target argument counts
   of every terminator owned by the machine, every opcode-10 explicit argument
   count owned by the machine, and zero;
5. scratch begins at the next eight-byte-aligned cursor and occupies exactly
   eight bytes per scratch cell; and
6. the complete frame is rounded to 16-byte alignment.

Let `V(v)` and `P(p)` retain the CKIR1 meanings for the positive frame
displacements of value and place slots, and let `C` be the positive displacement
immediately before the first scratch cell. A call with `n` explicit arguments
uses exactly the cells at displacements `C+8` through `C+8n`. For argument
ordinal `j`, zero based, define:

```text
A(j,n) = C + 8 * (n - j)
```

The caller emits argument stores in increasing ordinal while their frame
displacements decrease: argument zero is at `[rbp-A(0,n)]`, the lowest address,
and argument `j` is consequently at `[rsi+8*j]` after `rsi` is initialized.
This reversal is canonical. It is not an implementation-selected stack order.
The same scratch extent is reused by state edges and later calls only after the
current operation has consumed it.

Every individual frame is at most 262,144 bytes. A native call contributes the
eight-byte return address and the callee prologue contributes one saved eight-
byte `rbp`, so the selected root starts with live-stack cost
`frame(entry)+16`. In topological call-graph order, an edge from caller to callee
has candidate live cost:

```text
live(caller) + frame(callee) + 16
```

The greatest predecessor cost is retained for each callee, and every retained
cost is at most 262,144 bytes. Thus the selected root frame is at most 262,128
bytes. This is a live-stack exhaustion check over the complete finite DAG, not
a recursion depth or runtime fuel limit.

### Caller staging and opcode-10 bytes

Using the CKIR1 helpers `LV(v)`, `SV(v)`, `LP(p)`, and `REL(target)`, the caller
stages explicit arguments `v0..v(n-1)` in source/parameter ordinal order. A
scalar argument `vj` emits:

```text
LV(vj); 89 85 -A(j,n)
```

A structural argument emits:

```text
48 8B 85 -V(vj); 48 89 85 -A(j,n)
```

Only the low four bytes of a scalar cell are defined and consumed. A structural
cell contains the complete eight-byte immutable address. After all argument
cells are staged, operand-zero receiver place `p` and the argument-block pointer
are installed exactly as follows:

```text
LP(p); 48 89 C7                         mov rdi,rax
31 F6                                   xor esi,esi              when n = 0
48 8D B5 -A(0,n)                        lea rsi,[rbp-A(0,n)]      when n > 0
E8 REL(callee entry block)              call callee
```

For a Unit call, those bytes complete the operation. For a scalar-result call,
the callee returns the checked result in `eax` and the caller appends
`SV(result)`. There is no second caller-side range check: the exact callee result
type equals the call result type, and every callee `ReturnValue` performs the
declared-result check before its epilogue. A Unit return leaves `eax`
uninterpreted.

Arguments are fully materialized before staging, so a nested call completes
before its enclosing call writes these cells. The caller frame and every
structural argument's source object remain live for the complete synchronous
callee invocation. The callee cannot mutate a structural parameter through its
value slot or retain its address through a structural result, because neither
operation exists in CKIR2.

### Callee prologue, parameters, and result

Every reachable machine's entry block, including the selected entry, begins
with the exact prologue:

```text
55                              push rbp
48 89 E5                        mov rbp,rsp
48 81 EC frame_size             sub rsp,frame_size
48 89 BD F8 FF FF FF            mov [rbp-8],rdi
```

The selected entry has no explicit parameters and therefore does not read
`rsi`. For an ordinary callee, machine parameters are then installed in
parameter ordinal. Let parameter `j` have value ID `v` and exact type `t`. A
scalar parameter emits:

```text
8B 86 (8*j); CHECK(t); SV(v)
```

A structural parameter emits:

```text
48 8B 86 (8*j); 48 89 85 -V(v)
```

The scalar load reads the low four bytes of its eight-byte argument cell and
checks the destination parameter's exact interval before committing its value
slot. The structural load copies the immutable address, not the aggregate's
semantic leaves. After parameter installation, the entry block emits its
ordinary operations and terminator; non-entry blocks of the same machine reuse
the active frame and do not repeat the prologue.

`ReturnUnit` emits exactly `C9 C3`. `ReturnValue(v)` emits
`LV(v); CHECK(machine result type); C9 C3`, leaving the checked scalar in `eax`.
Calls target the callee's exact entry-block offset with signed rel32. Reachable
machines are emitted in increasing machine-ID order and their blocks in block-
ID order, so both sizing and final emission reconstruct the same call target.
Every frame size, scratch base, parameter displacement, result slot, live-stack
cost, instruction length, and call displacement is computed and checked before
the first ELF byte is published.

## Finite acyclic call graph

Every Call row contributes an edge from its owner machine to its callee. The
complete graph over all machine rows must be acyclic, including machines not
reachable from the selected root. Self calls and longer internal or
cross-source cycles reject 251. This tranche has no recursive call form or
runtime recursion limit.

The first resolver/lowerer surface admits ordinary same-owner
`self.name(arguments)` calls, including calls to a machine declared in another
source of the same semantic module. Broader receiver/member and cross-package
call syntax requires a later resolver ruling and is not implied by opcode 10.

## Resources

All CKIR1 ceilings remain unchanged, including the explicit aggregate ceiling
of 94,208 operand-vector words and 2,260,040 encoded CKIR bytes. The operand
ceiling covers operation operands and terminator edge operands together. Call
arity may reach eight operands, but the product does not claim the larger
all-operations-at-maximum-arity structural bound. Crossing either frozen
aggregate limit rejects 252 before publication.

Malformed identity, root, binding, target, signature, order, graph, or result
relations reject 251 without output. A well-formed input exceeding a published
ceiling rejects 252 without output. All checks, including whole-module cycle
and role-3-consumption checks, complete before the first CKIR byte is written.
