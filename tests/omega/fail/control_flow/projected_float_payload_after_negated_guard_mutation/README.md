# Final negated guard operand replaces the selected case

The true guard first observes `Reading`, then replaces it with `Empty` in a
call whose false result is compared with `false`. That final comparison cannot
be omitted to revive the earlier selected-case fact. The payload access must
reject for missing live case authority.

Run `python3 tools/corpus_gate.py --filter control_flow/projected_float_payload_after_negated_guard_mutation`.
