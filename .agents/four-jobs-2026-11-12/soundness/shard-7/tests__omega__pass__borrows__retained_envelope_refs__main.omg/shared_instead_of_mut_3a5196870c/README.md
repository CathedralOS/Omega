# retained_envelope_refs

Retained references inside a generic envelope: `Envelope<&i32>` stores
`&other_backing` and even `&retained` — a reference to a reference cell —
while `let writable: &write i32 = &write write_backing` makes a write-only
local loan.

## Learnings

- Forwarding a `&write` local needs the explicit `&write writable`
  attenuation spelling at the call site — a bare forward reads it.
- `e.first` (an `&i32` member) reads through the reference in `as i32 in
  Wrapping` arithmetic position.

Compile-checked only: freestanding — run a check target, not native.
