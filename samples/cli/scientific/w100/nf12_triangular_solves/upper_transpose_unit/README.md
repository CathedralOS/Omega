# Upper transpose, backward substitution and unit diagonal

U = [[2,-1,3],[0,4,2],[0,0,-2]]. Solve U^T X=B for B columns
[4,2,14] and [-2,13,-1]. Expected X columns are [2,1,-3] and [-1,3,2].
Every solution and transposed residual is checked. Ordinary U backward solves
use B columns [-6,-2,6] and [1,16,-4], with the same solutions.
A unit-upper system stores all three diagonal cells as zero, skips them and
solves B=[-8,-5,-3] to X=[2,1,-3]. A final negative-zero pivot must return
ZeroPivot explicitly. Exit 0 and the PASS stdout line are the oracle.

Run the explicit preparation step and commands in the parent NF12 README.
The general descriptor-driven API remains pending dependency signatures.
