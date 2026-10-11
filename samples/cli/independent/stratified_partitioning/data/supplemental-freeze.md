# Supplemental oracle freeze

This extra variant was independently frozen before its first execution; the
original 70-case manifest and its pre-execution hashes remain unchanged.

Fifteen rows belong to group 1, and one row (ID 99) belongs to group 8;
all sixteen belong to stratum 1. At fraction 1/9 the only proper subsets
have scaled errors 119 (group 1) or 7 (group 8). Group 8 must win, with
mask 128, train 15, test 1, score 7 and total error 7. Empty strata have
zero error. This exercises a selected highest group bit and a three-digit
mask, beyond the original cases. Tolerance is zero. The hand report and
independent combination enumerator agreed before execution.

Supplemental manifest SHA-256: `7893a5e0a5bea6897fd3dab01a2ff2e4d1a25ff08afd17beb1ff9896dee0dd4b`.
