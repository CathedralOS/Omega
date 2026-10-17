# Matched field windows

The shared fact and operation in every place: a matched window has length 2,
and `write_pair` writes its original backing.

A match arm treats its integer pattern as the subject's value. In the arm
`which == 0`, the window `items[0..which + 2]` has length 2, the same length
as the other arm, so the returned window publishes length 2. Each window is
written through a parameter that requires length at least 2.

The same fact is one program per place:

```sh
omega run samples/cli/facts/matched_field_windows/local/main.omg
omega run samples/cli/facts/matched_field_windows/field/main.omg
omega run samples/cli/facts/matched_field_windows/parameter/main.omg
omega run samples/cli/facts/matched_field_windows/call_result/main.omg
omega run samples/cli/facts/matched_field_windows/match_join/main.omg
omega run samples/cli/facts/matched_field_windows/const_name/main.omg
omega run samples/cli/facts/matched_field_windows/other_state/main.omg
omega run samples/cli/facts/matched_field_windows/other_machine/main.omg
omega run samples/cli/facts/matched_field_windows/target_body/main.omg
```

Expected exit: 0.

`tests/omega/fail/slices/matched_open_window_end` is the wildcard arm, which
does not learn that value and must reject the unproven end.

Every place has a reject twin at
`tests/omega/fail/facts/matched_window_through_<place>_rejected`. The twin
changes one selected extent from 2 to 1 while retaining the required length 2.
The target-body twin changes only Windows, which must reject on every host.
