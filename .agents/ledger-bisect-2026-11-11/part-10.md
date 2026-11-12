# Ledger bisect part 10

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## PIN cluster summary

| cluster | rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| pin-c1 | 15 | pass/run rejected | native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(3) ha | `pass/build/default_component_abi_extents` | `` |  |  | pending |
| pin-c2 | 1 | pass/run rejected | anonymous address value cannot prove fit in `addr::Bound` in machine `SelectedImage::retained_suffix | `pass/build/explicit_provider_storage_extent_exit` | `55f4a84e523c` | 1074 | psi: preserve indexed observations through normal call completion (#1074) | bisected |
| pin-c3 | 1 | pass/run rejected | cannot establish assignment invariants: partial write enclosure has no exact active-case frontier | `pass/borrows/returned_record_case_field_store_exit` | `3a76781c7ff3` | 2068 | psi: preserve bounded writes through mutable byte recasts (#2068) | bisected |
| pin-c4 | 1 | pass/run rejected | const argument expression in generic data `ForwardedBuffer` is invalid: const operator application r | `pass/generics/runtime_const_data_symbolic_expression_exit` | `` |  |  | propagated |
| pin-c5 | 1 | pass/run rejected | machine call cycle: Caller::read... | `pass/traits/applied_requirement_lexical_isolation_exit` | `` |  |  | pending |

## Members

### pin-c1 (15 rows)
- `pass/build/default_component_abi_extents` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(3) has no admitted mechan
- `pass/build/native_component_case_cycle` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(3) has no admitted mechan
- `pass/build/native_component_case_loan_cycle` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(3) has no admitted mechan
- `pass/build/native_component_reuse` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(3) has no admitted mechan
- `pass/components/independent_same_spelling_packages` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(3) has no admitted mechan
- `pass/facts/component_scalar_lending_through_call_result_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/facts/component_scalar_lending_through_const_name_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/facts/component_scalar_lending_through_field_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/facts/component_scalar_lending_through_local_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/facts/component_scalar_lending_through_match_join_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/facts/component_scalar_lending_through_other_machine_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/facts/component_scalar_lending_through_other_state_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/facts/component_scalar_lending_through_parameter_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/facts/component_scalar_lending_through_target_body_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(4) has no admitted mechan
- `pass/providers/independent_imported_data_exit` (broken) native artifact terminal-authority closure review failed: foreign settlement BoundaryMachineId(3) has no admitted mechan

### pin-c2 (1 rows)
- `pass/build/explicit_provider_storage_extent_exit` (broken) anonymous address value cannot prove fit in `addr::Bound` in machine `SelectedImage::retained_suffix_start` state `retai

### pin-c3 (1 rows)
- `pass/borrows/returned_record_case_field_store_exit` (broken) cannot establish assignment invariants: partial write enclosure has no exact active-case frontier

### pin-c4 (1 rows)
- `pass/generics/runtime_const_data_symbolic_expression_exit` (broken) const argument expression in generic data `ForwardedBuffer` is invalid: const operator application requires declaration 

### pin-c5 (1 rows)
- `pass/traits/applied_requirement_lexical_isolation_exit` (broken) machine call cycle: Caller::read...

## Fixed on PIN (prior slice rows)

- `fail/build/explicit_provider_scalar_budget`
- `fail/build/explicit_provider_storage_extent`
- `pass/capabilities/selected_applied_synchronous_chain_exit`
- `pass/drops/runtime_selected_partial_owner_cleanup_exit`

## Commands

- Direct measure: `OMEGA_CORPUS_FIXTURE_FILTER=... corpus_runner`
- Bisect: `.agents/ledger-bisect-2026-11-11/tools/bisect_one_cluster.sh`
- Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`
