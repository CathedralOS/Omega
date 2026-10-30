# Computed runtime data indices

A runtime limit from 9 to 10 binds `Index<(origin + 1)>`, where ordinary flow
facts establish `origin = limit - 1`. The application captures that descriptor
before the constructor payload call overwrites `origin` with 4294967295.
Observation checks payload 8, the retained limit, and the overwritten source.
Every invalid twin changes only the payload to 10.

All nine transport places have exact positive and intended-invalid corpus
mirrors. Explicit local type formation uses the facts available before the
initializer; inferred and operand-capture types keep their existing captured
result context. Earlier writes and lost join facts cannot establish arithmetic
formation. Const operands retain their static formation requirements.

The governing contracts are runtime index identity and storage in generics.md,
ordinary Exact arithmetic formation, selected operator meaning, and independent
source and consumer obligations. Native and Terminal observations must come
from actual matching-host execution. Full RUNTIME-VALUE-GENERICS remains open
for selected-operator indices, nested result composition, and cross-host work.
