# zero_len_array

`[u8; 0]` is first-class at check: locals, consts, params, returns, record fields, nested arrays; `[]`/`""` construct it exactly, compares by value, `.len` = 0. Fences live at use (no provable index, exact element count).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
