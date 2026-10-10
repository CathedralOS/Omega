# Tall noisy line fit

Fit intercept plus slope to coordinates [-1,0,1,2] and measurements [0,0,2,6].
The 4 by 2 design backing is [1,1,1,1,-1,0,1,2], leading dimension 4.
The unique solution [1,2] gives A*x-b = [-1,1,1,-1], squared norm 4.
Both components of A^T residual vanish exactly; det(A^T A) = 20 > 0.
With the chosen reflector signs, thin Q columns are [-1/2,-1/2,-1/2,-1/2]
and [3/2,1/2,-1/2,-3/2]/sqrt(5); R is [[-2,-1],[0,-sqrt(5)]].

The complete `main.omg` is an independent algebraic oracle witness. Run this
project's mathematical QR qualification with `python run_reference.py` from
this directory. The shared driver checks solution, explicit residual, transformed
RHS tail norm, normal residual, reconstruction and thin-Q orthogonality.

For the integrated NF15 sample, caller-allocate separate backing arrays: original
matrix 8, factor 8, RHS 4, transformed RHS 4, tau 2, reflector scratch 4,
solution 2, residual 4, thin Q 8. Create 4 by 2 column-major descriptors through
NF01. Call try_least_squares with immutable original and RHS and distinct mutable
factor/tau/scratch/transformed-RHS/solution/residual buffers; require FullRank.
Call try_qr_report with explicit thin-Q backing. Check every field against the
frozen JSON; verify original matrix and RHS still equal their authored values.
Do not materialize scratch as a view of factor or borrow input mutably.

The frozen native oracle is exit 0. Check, native and Terminal commands appear
in the parent README; these routes remain unqualified because resolver cache custody
rejects before source validation and the shared interfaces are absent. An algebraic witness is not a least-squares API acceptance.
