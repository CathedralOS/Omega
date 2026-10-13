# qualified_array_literal

`[u32;1] in Wrapping` literals retain element policy — nested `[[u32;1];1]` + record member literals; wraparound `values[0]+1` folds to 0.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.

## Gaps found while authoring (filed)

- Dynamic `&mut` slice-window locals: `let held: &mut [i32] =
  context.items[split_point..4]` reports "requires mutable source
  access" — corpus fixture `borrow_domain_window_write_and_call`
  fails `--check` identically (regressed-from pin).
