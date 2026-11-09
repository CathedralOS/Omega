# An indexed Empty guard survives helper calls

This is a smaller witness for Cathedral's paging migration. An exact Empty guard on one of 8192 slots must justify the same slot's requirement in `take` and `keep`; filling that slot transfers the new linear value into the pool. The invalid twin removes only the guard.

The runtime checks distinct 47 and 59 payloads, an occupied-slot fallback of 83, a still-Empty neighboring slot and 91/97 surrounding canaries. The Token is an ordinary linear value and supplies no physical-storage authority. Its drop hook does not establish a cleanup count or order.
