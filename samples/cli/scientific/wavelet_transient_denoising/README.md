# Haar transient denoising

An orthonormal Haar transform preserves the coarsest approximation, applies
soft thresholding to the requested number of finest detail levels, then
reconstructs the supplied signal. The implementation uses pairwise butterflies;
each stage stores its approximation/detail pairs in fixed-size scratch rows.
A helper returns each selected row as an independent owned packet before
reassembly writes the coefficient buffers. The independent oracle uses a dense
matrix defined directly by Haar support and
sign, with Decimal square roots at 80 digits.

Supply whitespace-separated decimals, terminated by EOF:

```
length threshold finest_levels
sample ...
```

Lengths are 8, 16, 32 or 64. Levels are integers from 0 through log2(length).
Threshold is nonnegative. Every input value has magnitude at most 1000, at most
13 digits total and at most nine fractional digits. A minus sign is optional;
a decimal point needs digits on both sides. ASCII space, tab, CR and LF separate
tokens. The complete stream must be shorter than 4096 bytes. Missing/extra tokens,
unsupported lengths, fractional/out-of-range levels, negative thresholds,
nonfinite/exponential spellings and malformed/over-budget data print
`ERROR input or options` and exit 10. Valid input exits 0. The bounds prevent
arithmetic overflow and keep numerical output inside signed i64. A closed stdin
is required for automated runs.

Coefficients are ordered A0, Dcoarsest, ..., Dfinest; details within each level
follow left-to-right support. Selected detail values become
sign(c)*max(abs(c)-threshold,0); equality yields zero. Level zero leaves every
coefficient unchanged. The approximation is never shrunk. Output has alternating
`original_pico` / `retained_pico` rows for each coefficient, then
`reconstruction_pico` for each sample. Divide those signed integer values by
1e12. Final `input_energy_nano`, `coefficient_energy_nano` and
`removed_energy_nano` rows are divided by 1e9. Output truncates toward zero.
Removed energy is the pre-shrink coefficient energy minus retained coefficient
energy; it is the energy lost by shrinkage, rather than squared residual energy.

From repository root, using the documented environment setup:

```
omega update --project samples/cli/independent/wavelet_transient_denoising --target linux_x86_64
omega run samples/cli/independent/wavelet_transient_denoising/main.omg < samples/cli/independent/wavelet_transient_denoising/input.txt
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p wavelet-transient-validation
python3 samples/cli/independent/wavelet_transient_denoising/validate.py --runner /absolute/path/to/wavelet-transient-validation --evidence /absolute/path/to/evidence
```

`fixtures.json` is frozen before program execution. `validate.py` recomputes
that independent oracle, checks actual native and captured Terminal values within
1e-8, checks zero-threshold roundtrips, approximation preservation and pre-shrink
Parseval within 1e-9 of max(1,input energy). It also checks numerical removed
energy and exact refusal output. `validation/` prepares the normal project and
captures both products of the same retained Terminal artifact without bypassing
package or root authority. Its isolated containing-domain exit delegation is the
normal CLI policy. The separate `controls/detail_bound/main.omg` must reject
at source checking for its statically invalid detail access.

Build binds explicit Linux x86-64, Windows x86-64 and macOS ARM64 entries.
Execution qualification records the actual host in VALIDATION.md.
