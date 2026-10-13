# write_domain_store

Write-only whole-leaf replacement discharges the stored value's domain fact: `tagged.label = label` proves `in Utf8` write-side like `&mut`; `in Wrapping` fields owe no range proof.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.

## Learnings

- `&write` projection grants NO observation: `policy.count + 1`
  (read-modify-write) is refused — replace-only.
- A record with a domain-bearing field must hold a proven member value
  at the return boundary; an uninitialized `outer: Outer` Main field
  failed the default-domain proof, so `let mut` locals + `&write`
  establishment is the working shape (the store itself proves `in
  Utf8` write-side).
