# Forward substitution with multiple RHS

L = [[2,0,0],[3,1,0],[-1,4,2]]. B's columns are [2,1,-3] and
[-2,0,17]. Expected X columns are [1,-2,3] and [-1,3,2].
The sample checks all six elements and all six reconstructed residuals.
It then replaces the final diagonal with zero and requires preflight rejection
with the input RHS unchanged. Exit 0 and the PASS stdout line are the oracle.

Run the explicit preparation step and commands in the parent NF12 README.
The general descriptor-driven API remains pending dependency signatures.
