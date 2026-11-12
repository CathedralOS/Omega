# projected_subject_measure

A declared scalar view over projected storage:
`countdown.remaining -> Countdown::Doubled` produces `value * 2`
from the exact `u64` member coordinate rather than a bare formal.
The flat self call descends directly; the two-state form transports
the same coordinate and the pinned ceiling — the extra requires
`countdown.remaining * 2 <= limit` bounds the produced rank.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
