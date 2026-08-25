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
