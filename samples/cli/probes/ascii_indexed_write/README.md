# ascii_indexed_write

Indexed write through a qualified byte owner:
`bytes[at] = value` on `&mut [u8; 8] in Ascii`
succeeds because `value < 128` dominates the store and
re-proves `ascii_only` per element. Also: named states
declare `-> bool` results like machines, and a state
re-enters itself on a `repeat` edge.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
