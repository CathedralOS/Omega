# Omega Canaries

Canaries are tiny programs that isolate one compiler capability at a time.
They are not samples, tutorials, or end-user examples.

Current feature canaries:

1. `state_transition_chain` checks explicit state-to-state flow.
2. `nested_machine_continuation` checks nested machine call and continuation flow.
3. `owned_assignment_before_exit` checks owned data mutation before process exit.
4. `guarded_transition_dispatch` checks ordered transitions from parsed command data.
5. `mutable_output_host_call` checks host calls that write through mutable output data.
6. `record_array_field_access` checks simple records, arrays, and field reads.
7. `runtime_text_storage` checks dynamic text slots fed by literals and console input.
8. `guarded_leaf_branch_expansion` checks small guarded helper states with leaf-body writes.
