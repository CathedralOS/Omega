# Bounded cubic powder diffraction indexing

Input is whitespace-separated `wavelength peak_count angular_tolerance` followed
by 3–6 strictly increasing **2θ angles in degrees**. Wavelength is 0.5–2.5 Å,
angles lie strictly between 0 and 180°, and tolerance is strictly positive and
at most 1°. Decimal syntax permits a sign and decimal point, at most 18 digits
and 12 fractional digits per token, nine tokens, and fewer than 2048 bytes.
NaN, infinity, exponents, extra tokens and malformed decimals reject.

The application enumerates primitive (class 0), body-centred (class 1), and
face-centred (class 2) cubic lattices. Canonical families have h≥k≥l≥0 and
N=h²+k²+l²>0. I requires h+k+l even; F requires equal parity of all components.
Only the first eight distinct allowed N values enter the search. At most two
unassigned lines **before the highest observed rank** may be missing; the
unmeasured tail beyond the final peak is not counted as missing. Observations
must map in order to distinct allowed N. Every admissible assignment is emitted,
including multiple lattice classes and multiple families sharing one N.

The crystallographic scattering vector is q=2 sin(θ)/λ in Å⁻¹ (without 2π).
An unweighted least-squares fit through the origin gives scale=ΣNq²/ΣN²=1/a².
The fitted lattice a must lie in [2,10] Å; every assigned first-order Bragg angle
must be real and every signed predicted-minus-observed 2θ residual must be within
tolerance. This fit minimizes squared-vector residuals; angular residuals are
an independent admission test. Lattice class is a hypothesis: atomic-basis
extinctions, intensities, strain, zero-offset calibration and noncubic structures
are outside this bounded model.

Output begins with its units. `hypothesis class lattice scale` opens a record;
`peak N predicted_2theta residual` is followed by every exact integer `family h k l`;
`missing N` lists each missing line; `end` closes the record. Lattice, reciprocal
scale, angles and residuals are signed integers in 1e-12 units. The final
`hypotheses count` reports the full ambiguity set. Exit 0 means at least one
hypothesis; exit 2 means valid but incompatible input (count zero); exit 64 means
invalid input with `error invalid input`. Exit 70 is an internal decimal-output
invariant failure and must never appear on admitted scientific inputs.

Run with the normal product route (activate the documented toolchain first):

```sh
omega update --project samples/cli/independent/powder_diffraction_indexing --target linux_x86_64
omega run samples/cli/independent/powder_diffraction_indexing/main.omg < samples/cli/independent/powder_diffraction_indexing/input.txt
python3 samples/cli/independent/powder_diffraction_indexing/validation/validate.py --omega /absolute/path/to/omega --route all
```

Complete any generated exact package decisions through ordinary review/resume.
The validator compiles its auxiliary observer from the same producer's retained
libraries, publishes the actual admitted native product, captures native and
canonical Terminal numerical output, and compares both with immutable independent
70-digit expectations within 1e-7. Its per-case wall clock and explicit sponsor
fuel bound execution. Source-invalid reflection rank 8 must reject before runtime.
`validation/oracle.json` and its input hashes were frozen before Omega execution.
`validation/freeze_reference.py` uses signed enumeration and combinations,
independent of the application's canonical-triple and bitmask enumeration.

Scientific references: the IUCr dictionary gives [Bragg's law](https://dictionary.iucr.org/Bragg%27s_law)
and the P/I/F [integral reflection conditions](https://dictionary.iucr.org/Reflection_conditions).
The cubic spacing relation d=a/√N yields the reciprocal-scale model above.
