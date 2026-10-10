# selector_once

First sample proving the index selector evaluates once before
its replacement call: `tokens[select(&mut cursor)] =
replacement(&mut cursor)` — `select` mutates `cursor.index`
to 1 but the store still lands at the saved coordinate 0;
`ensures result == 0` contract on the selector.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
