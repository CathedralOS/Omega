# wire_retired_field

Data-level `retired #3;` seals a published field identity — distinct
from `retired #N` inside a numbered sum-case payload — so a later schema
edit cannot silently reuse `#3`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
