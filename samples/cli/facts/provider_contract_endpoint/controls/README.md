# Endpoint formation and transport controls

These controls belong to the same selected endpoint, captured bound and exact
subtraction as the nine-place row. They challenge the prerequisites introduced
by this repair rather than adding transport matrices.

- `circular_endpoint_formation` rejects a helper that uses its own pending
  result to admit a call. The [pending-call examples](../pending_call/README.md)
  exercise the same requirement across all nine places.
- `crashing_endpoint` must reject a helper before constant evaluation because
  it can crash. An incomplete lowering error or an interpreted trap does not
  prove admission. The [guarded-crash examples](../guarded_crash/README.md) add
  safe calls and zero-divisor twins; fresh qualification is still required.
- `false_result_guarantee` and `mutated_scalar_return` reject an actual returned
  value outside the selected endpoint. The source-model test
  `identical_runtime_calls_do_not_identify_distinct_static_denotations` also
  distinguishes raw constant actuals and retained applications after erasure;
  runtime call shape alone never identifies closed denotation.
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
  `generic_forwarded_provider_selection` execute ordinary computed mode,
  forwarded original Build identity and two closed generic helper instances
  through the landed source-issued Product selection protocol. The raw
  `direct_provider_protocol` call lacks that compiler-issued request and rejects.
- `public_header_private_endpoint` rejects exposing a private endpoint.
  `runtime_self_forbidden`, `machine_contract_self_forbidden` and
  `nested_self_forbidden` retain predicate-only logical-subject scope.
- `composed_endpoint` and `indirect_helper` execute ordinary transitive checked
  calls rather than a recognized endpoint spelling.

The [earlier-operand preservation control](earlier_operand/README.md) keeps an
unused constant's provider call and later exact addition in the same evaluation
entry. Its [source](earlier_operand/main.omg) and [Build](earlier_operand/build.omg)
match the frozen passing R6 probe. The constant still owes formation, while main
checks the original bound seven; it does not directly observe the resulting eight.
Current-candidate qualification remains pending.

Historical generic-endpoint/static-actual and portable-family circular source
experiments reached earlier unsupported formation. Their exit codes are not
credited as enforcement of their later intended obligations. Their raw inputs
and diagnostics remain in the qualification record. The direct ordinary
circular control and the static-metadata source-model counterexample provide
the applicable observations for those implementation premises.
