# Periodic regularized deconvolution

Read `n m lambda`, then `m` kernel coefficients and `n` measurements from stdin.
`n` is an integer in 8..32, `m` is 1, 3 or 5, and lambda is in 0.0001..100.
Kernel coefficients and measurements are nonnegative. The kernel sum must be
within 1e-12 of one. Input uses signed decimal tokens with optional fractional
parts, at most 100 digits per token and values at most 1e100. ASCII space, tab,
CR and LF separate tokens. Exponent notation, plus signs, NaN, infinity, incomplete
tokens, excess tokens, and streams of 4096 bytes or more are refused with exit 2.
The byte budget includes trailing whitespace and is enforced while reading.

The application minimizes `||Hx-y||² + lambda ||Dx||²` without a positivity
constraint on x. Boundaries are periodic:
`Hx[i] = sum(kernel[j] * x[(i + m/2 - j) mod n])`, and
`Dx[i] = x[(i+1) mod n] - x[i]`. Conjugate gradients solve the positive definite
normal equations. A failure to converge within 256 iterations exits 3.
Signed reconstruction and reconvolution fields appear in index order, followed
by residual norm `||Hx-y||`, roughness `||Dx||`, and the objective. Scientific
notation retains 13 significant decimal digits. Negative reconstruction is
reported, never clipped.

```sh
omega packages update --project samples/cli/independent/regularized_deconvolution --target linux_x86_64
omega run --both --keep samples/cli/independent/regularized_deconvolution/main.omg < samples/cli/independent/regularized_deconvolution/input.txt
```

The optional `--batch-runner` argument retains compatibility with the earlier
qualification example; normal validation needs only the CLI.
