A matched integer is substituted into each arm of a returned record window.
The arms publish different lengths, and `result.events.len >= 1` holds for
both. Callers observe the window through a local, a nested field, a
parameter, and a call result. A field subject uses the same substitution.
The wildcard arm in `fail/slices/matched_open_window_end` still learns nothing.
