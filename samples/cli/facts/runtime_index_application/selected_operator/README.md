# Selected operators in runtime data indices

Each application captures `origin + 1` through the selected Arithmetic contract,
whose result equals its left operand. RetainLeft therefore preserves limits 9
and 10; builtin addition would produce 10 and 11. A payload call then overwrites
origin with 4294967295. Observation checks the selected descriptor, payload 8,
and the source write. Each twin selects payload_bad, which performs the same
write and supplies 10, violating the default domain.

All nine transport places and exact positive/reject corpus mirrors are present.
Matching-host native/Terminal qualification is required; generated foreign images
supply no host execution evidence. Full RUNTIME-VALUE-GENERICS remains open.
