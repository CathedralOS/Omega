# Endpoint formation and transport controls

These controls check that a computed range limit stays valid when helpers call
other helpers or values move through records. They use the same selected limit
and exact subtraction as the nine-place examples.

- `circular_endpoint_formation` rejects a helper that uses its own pending
  result to admit a call. The [pending-call examples](../pending_call/README.md)
  exercise the same requirement across all nine places.
- `crashing_endpoint` rejects before constant evaluation and names the `Trap`
  route through `limit`. The [guarded-crash examples](../guarded_crash/README.md)
  run safe calls across all nine places and reject their zero-divisor twins
  before execution. An unrelated lowering error or an interpreted trap does not
  establish that admission check.
- `false_result_guarantee` and `mutated_scalar_return` reject an actual returned
  value outside the selected endpoint. The source-model test
  `identical_runtime_calls_do_not_identify_distinct_static_denotations` also
  checks that two calls with identical runtime shapes retain their distinct
  static arguments.
- `field_snapshot_before_sibling_write` preserves the value captured before a
  sibling mutates its source. `field_snapshot_after_sibling_write` rejects the
  changed value; `field_false_conjunct` cannot discard another default-domain
  requirement. The positive also checks that the sibling call changed its
  source to eight and left neighboring fields 31 and 47 untouched. The independent shared-handle source-model test keeps later
  scalar reevaluation distinct from an earlier captured child.
- `dormant_target_endpoint_positive` checks the provider's authored target
  bodies. Its reject twin changes only the dormant Linux endpoint to 6 and
  rejects the Linux body's captured input 7, even on a macOS primary target.
  `unrelated_pending_target_contract` does not treat an unrelated pending
  declaration as a demanded invocation.
- `computed_composition_mode`, `forwarded_provider_selection` and
  `generic_forwarded_provider_selection` check that computed and forwarded selections retain the Build identity that
  authorized them, including two distinct generic instances. The direct
  `direct_provider_protocol` call lacks a compiler-issued selection request and
  rejects.
- `public_header_private_endpoint` rejects exposing a private endpoint.
  `runtime_self_forbidden`, `machine_contract_self_forbidden` and
  `nested_self_forbidden` reject using `self` outside the predicates where it
  names the value being checked.
- `composed_endpoint` and `indirect_helper` execute ordinary transitive checked
  calls rather than a recognized endpoint spelling.

The [earlier-operand preservation control](earlier_operand/README.md) keeps an
unused constant's provider call and later exact addition in the same evaluation
entry. Its [source](earlier_operand/main.omg) and [Build](earlier_operand/build.omg)
match the frozen passing R6 probe. The constant still owes formation, while main
checks the original bound seven; it does not directly observe the resulting eight.
Current-candidate qualification remains pending.

Some older generic and circular-call experiments stopped before reaching the
call check they were intended to test. Their exit codes do not prove that check;
the qualification record preserves their inputs and diagnostics. The direct
circular control and the source-model test with distinct static arguments supply
the applicable evidence instead.
