# Matched field windows

A match arm treats its integer pattern as the subject's value. In the arm
`which == 0`, the window `items[0..which + 2]` has length 2, the same length
as the other arm, so the returned window publishes length 2.

The subject is observed as a parameter, a local, and a field. One arm takes
its end from a call that ensures the result is 2. Each window is written
through a parameter that requires length at least 2.

```sh
omega run samples/cli/facts/matched_field_windows/main.omg
```

Expected exit: 0.

`tests/omega/fail/slices/matched_open_window_end` is the wildcard arm, which
does not learn that value and must reject the unproven end.
