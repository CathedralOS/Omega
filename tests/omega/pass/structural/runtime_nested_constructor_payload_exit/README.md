# Nested constructor payload

`omega run --both main.omg` exits 0 natively and through Terminal Psi.

Completed signed and Boolean arrays, including a nested array and an empty
byte array, become record fields. Those records then become array elements and
a selected case payload returned by a helper. Case selection carries the
payload through a block argument and an owned call. Mutating an independent
copy must not change the caller's saved primitive or Boolean cells.

Copyability is explicit. `fail/borrows/affine_record_copy_handoff` must still
reject reuse of a moved record; nesting cannot turn affine custody into a copy.
