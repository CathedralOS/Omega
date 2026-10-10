# Correlated SPD with explicit copied work backing

A=[[4,2,4],[2,10,-1],[4,-1,9]], b=(12,-21,33).
L=[[2,0,0],[1,3,0],[2,-1,2]], y=(6,-9,6), x=(1,-2,3).
Stored column-major, the off-diagonal L entries produce cross terms, unlike
the diagonal example. Source is borrowed readonly during an explicit element
copy; factorization overwrites only copied work backing. Forward solve computes
y, then backward transpose solve computes x. All source entries are checked.

Exit 0 requires every factor entry, solution, residual zero, preserved source,
and runtime data controls: nonsymmetric [[4,2],[1,3]], indefinite [[1,2],[2,1]]
(second pivot -3), and semidefinite [[1,1],[1,1]] (second pivot zero).
The controls classify exact two-by-two input data in a private fixture helper;
they do not establish the missing shared NF14 error representation.
Exact equality is stronger than the frozen 1e-12 limits for these exact inputs.
No user input is required. The parent README lists qualification commands.
