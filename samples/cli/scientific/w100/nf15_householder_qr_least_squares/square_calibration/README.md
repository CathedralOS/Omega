# Square calibration system

Solve [[3,1],[4,2]] * x = [5,6]. Column-major backing is [3,4,1,2], leading
dimension 2. The exact unique solution is [2,-1]; det(A) = 2 and residual zero.
The first reflector has v = [1,1/2], tau = 8/5, and beta = -5. It sends [3,4]
to [-5,0]. With the second reflector, Q = [[-3/5,4/5],[-4/5,-3/5]],
R = [[-5,-11/5],[0,-2/5]]. The determinant witness and reflector check
exercise different mathematics from the noisy fit's stationary residual.

The complete `main.omg` is an independent algebraic oracle witness. Run
`python run_reference.py` here for this project's frozen mathematical QR
qualification, including signed Q/R entries, reconstruction and orthogonality.

For integrated NF15 acceptance, caller-allocate distinct original matrix 4,
factor 4, RHS 2, transformed RHS 2, tau 2, reflector scratch 2, solution 2,
residual 2 and thin-Q backing 4. Make both 2 by 2 column-major descriptors
with NF01; call try_least_squares and require FullRank, then try_qr_report.
Check the frozen solution, zero explicit residual within tolerance, transformed
RHS tail norm exactly zero (empty tail), Q/R entries within 1e-12, and original
backing preservation. No reflector scratch may alias the input or factor.

Keep rank controls separate from this full-rank solve: dependent columns and
zero pivots return RankDeficientUnsupported, no solution and the failing pivot.
They must not be converted to a successful perturbed solve.

The frozen native oracle is exit 0. Check, native and Terminal commands appear
in the parent README. All Omega routes remain unqualified at this checkpoint.
