# Private arithmetic formation

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).

Every private arrival carries a positive value, so the helper can form value - 1.

The helper has no handwritten duplicate bound. Guards or named positive
constants establish the actual argument's lower bound; every admissible arrival
must preserve it. The operation is the same exact u64 subtraction in all places.
Each twin passes zero on one arrival while retaining the guard and operation.
This challenges lexical-name substitution as well as missing incoming facts.

The runtime oracles distinguish 7 -> 6 and 9 -> 8, with separate tags. Guarded
places also execute the zero fallback. The constant place supplies two named
positive constants. All four hosted entry bindings are authored.

Baseline `5ab4e12334fba74babfc3ae8d9992572fba307e6`, optimized producer,
refuses all nine positive places at `apply`'s `value - 1` initializer with
`exact arithmetic ... local result may overflow u64`. All twins reach that same
formation obligation. No caller guard or helper annotation has been added to
make a place pass. Implementation and end-to-end acceptance remain open.
