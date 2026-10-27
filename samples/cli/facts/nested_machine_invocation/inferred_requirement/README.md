# Private invocation requirements

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).

Every arrival establishes the equality consumed by the nested helper.

`probe` starts with arbitrary inputs. Its nested `forward` calls `same`, whose
published requirement is equality of its two explicit operands. Every arrival
supplies the same value twice; `forward` has no duplicate handwritten clause.
The two runtime paths retain distinct values (7 and 9) and tags (11 and 13).
The field place also observes unrelated canary fields. Each reject twin changes
only the second path's equality operand to 3, preserving the calling shape.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [missing equality](local/invalid/main.omg) |
| field | [source](field/main.omg) | [missing equality](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [missing equality](parameter/invalid/main.omg) |
| call_result | [source](call_result/main.omg) | [missing equality](call_result/invalid/main.omg) |
| match_join | [source](match_join/main.omg) | [missing equality](match_join/invalid/main.omg) |
| const_name | [source](const_name/main.omg) | [missing equality](const_name/invalid/main.omg) |
| other_state | [source](other_state/main.omg) | [missing equality](other_state/invalid/main.omg) |
| other_machine | [source](other_machine/main.omg) | [missing equality](other_machine/invalid/main.omg) |
| target_body | [source](target_body/main.omg) | [missing equality](target_body/invalid/main.omg) |

Each positive binds all four hosted targets. Actual observations are macOS ARM64
native and Terminal Psi executions; the other three hosts are unverified here.
Copies and authored exit/rejection oracles live in `tests/omega/{pass,fail}/facts/`
as `nested_machine_inferred_requirement_<place>_{exit,rejected}`.

Establishment and composition: entry contracts and ordered completed actuals
supply each helper's premises. Calls and terminal expressions use the same body
frontier. Three-level nesting and lifted constant binders carry exact declaration
identities without runtime captures. Every relevant direct source arrival must
be accounted for, including an otherwise unvisited one; unknown dispatch cannot
be silently excluded. The helper's body never imports its lexical parent's entry
contract. A private cycle avoiding the declared root, an escaping helper or an
exposed entry needs an independently established contract.

Identity and lifetime: scalar actuals are evaluated once and retain their saved
values. This row transports immutable scalars; it does not establish mutable
borrow, restoration or linear-custody acceptance. Existing mutation witnesses
remain required for those obligations.

Soundness controls:

- [Public declaration](controls/public_contract/main.omg) retains its own entry
  contract. [Removing it](controls/public_contract_missing/main.omg) rejects even
  when the only known caller supplies equal operands.
- [Unrelated callback](controls/unrelated_selection/main.omg) cannot reach the
  queried helper and does not poison its closed arrival inventory.
- [Escaping helper](controls/escaping_selection/main.omg) is selected through a
  callback signature that publishes no equality premise and must reject.
- [Proof-only constructors](proof_only/main.omg) exercise the original recursive
  proof-data case with a hosted entry. Its [twin](proof_only/invalid/main.omg)
  still owes equality for arbitrary `visit` inputs. Independently admitted erased
  citations retain source proof/custody records but own no runtime constructor
  storage, including resultless proof operations without an `ensures` clause.

Terminal/native lowering, constructor layout, calls, storage, encoding and image
emission are exercised by the executable record results. This row does not close
all target/ABI requirements, prove portable evidence for every computed ranking,
or qualify the entire declaration migration. The parent task retains the
Omega-written parser/resolver, linked customer migration, full affected corpus
migration and remaining invocation/custody/stack acceptance. Private requirements
through opaque or effectful bodies remain conservative rather than assumed.

Call-premise inference does not yet supply arithmetic-formation obligations
inside an unannotated private helper. The parent task retains that gap (for
example, proving `value - 1` from every guarded predecessor). The recursive
precondition control publishes the helper bound and spells the closing edge as
an explicit terminal transfer so only the invalid root invocation is at issue.
