# selector_window_debt

First sample mutating a selector during an open element
window: the extraction `tokens[at]` pins its coordinate —
`reset`/`at = 2` mutations afterward create no debt, and the
restore lands on the saved coordinate.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
