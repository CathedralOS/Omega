# Selected terminal operands retain structural call occurrences

A selected terminal call accounts its returned-record scalar operand once.
Cathedral's gallery header reads `previous_rect(...).y` inside a scalar argument
of a guarded terminal call. The ordinary statement route discovers that retained
record receiver, but the selected arm's scalar occurrence ledger omitted it.
The frozen optimized `b98730cd` producer refuses this regression with the same
`terminal call arm: operand occurrences` diagnostic as the gallery.

The positive observes a counter, its returned record field and both selection
outcomes. Omitting or duplicating the nested call, or evaluating the unselected
arm, changes the authored result. Its [linear twin](../../../fail/calls/guarded_terminal_linear_record_field/main.omg)
changes only the result carrier multiplicity and still rejects its unretired
owner. Occurrence accounting grants no cleanup or access authority.

Ordinary and selected scalar receivers must use the same checked structural
value graph inventory, preserving exact source call identity and nested operand
calls. This regression supports the real customer; it does not close the broader
call, ownership or machine capabilities.
