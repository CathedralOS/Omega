# Preserve the standing resident ceiling

These are another production of the existing atomic-fetch fact, not a new row.
An unsigned AND cannot increase either operand. A valid resident at most 100
therefore remains at most 100 even when the helper permits masks through 127.
The actual call still supplies mask 63, starts from 99, observes 35 and consumes
`100 - observed` before checking result 65.

The original mask-127 controls were valid programs that the compiler refused:
its common-bit-prefix approximation widened the resident ceiling from 100 to
127. They are retained here as positive programs. The shared integer owner now
keeps each unsigned operand's numeric ceiling as well as its known bits. No
atomic-specific bound rule or earlier observation is substituted for that law.

Each place has an `invalid/` twin that changes only the initial store from 99
to 127. This violates the unchanged resident bound 100 before the fetch occurs.
A shared atomic store must establish a valid resident immediately; a later AND
cannot excuse that earlier invalid state. The standard places use the same
establishment twins. All original sources, old refusal oracles and observations
remain in the frozen baseline evidence.

Owning task: **ATOMIC-MEMORY-MODEL**. See the [parent](../README.md) for complete
qualification and the remaining concurrent and refinement obligations. All nine
places and their establishment twins have actual observations on each of the four
matching hosts. The integrated Linux replay uses d322; the other hosts retain
their actual earlier compiler and program identities. The parent records those sources,
supplemental limits and remaining acceptance.
