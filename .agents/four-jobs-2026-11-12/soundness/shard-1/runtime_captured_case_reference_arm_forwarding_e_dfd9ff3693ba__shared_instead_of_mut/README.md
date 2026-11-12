# Captured case reference used after selection

After `omega update --project . --dangerously-accept-all`,
`omega run --both main.omg` must report native and interpreter exit 70.
A local shared descriptor captures a runtime-selected array element. The
selected arm reborrows that descriptor into an ordinary successor, which reads
the original referent again and combines its payload with the guard's snapshot.
Releasing the captured descriptor before evaluating the selected arm makes
that reborrow unavailable; a deferred transition release is not a guard-only
use. The program never mutates or outlives the borrowed array.
