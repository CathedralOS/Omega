# Sequential germination probability inference

This independent Bernoulli assay model assumes every seed has the same unknown
probability p, trials are independent conditional on p, and batches provide
counts of successes and trials. A Beta(alpha,beta) prior becomes
Beta(alpha+successes,beta+trials-successes) after each batch. It infers a common
unknown probability; it does not sample a fixed finite population or enumerate
pedigrees. Dependence, changing conditions and selection bias are outside this
model.

Supply unsigned decimal integer tokens separated by ASCII space, tab, CR or LF,
terminated by EOF:

```
prior_alpha prior_beta batches future_size
successes trials
... one pair per batch ...
```

Both prior parameters are 1–20. There are 1–8 batches, each with
0 ≤ successes ≤ trials ≤ 50. The future size is 0–32; zero produces the
single predictive bin P(K=0)=1. Zero-trial batches leave the posterior unchanged.
Each token has 1–3 digits (leading zeros are allowed within that bound).
The stream must contain fewer than 4096 bytes, including trailing whitespace.
Missing/extra tokens or records, signs, decimals, non-digits, invalid counts,
out-of-range parameters and oversized input print `ERROR invalid_input` and
exit 64. A complete valid stream exits 0. No inference output is emitted until
the entire input is validated. Automated execution must close stdin.

After each batch the program prints `alpha`, `beta`, then `mean_pico`,
`variance_pico`, `lower_pico`, `upper_pico`, and `above_half_pico`. The credible
limits are the equal-tail 95% Beta quantiles (0.025 and 0.975); `above_half` is
P(p>0.5), not P(a future majority). These five integer outputs are truncated
values scaled by 10^12. Divide them by 10^12. The final predictive distribution
prints alternating `bin` and `probability_pico` for K=0 through future_size,
followed by `status=ok`. Tiny values below 10^-12 print zero; the calculation
retains binary64 values before output conversion.

For positive integer shapes, the Beta CDF equals a Binomial(alpha+beta−1,x)
upper tail. The application sums nonnegative binomial masses using adjacent
ratios and reflects x>0.5 before calculation. The degree is at most 439;
the initial mass on the reflected side is at least 2^-439, above binary64
underflow. Quantiles use 48 bisections, a 2^-48 bracket before output
truncation. Prediction computes P(K=0) using rising-factor products, then the
beta-binomial adjacent-bin recurrence; it needs neither factorials nor huge
integers. Its denominator is positive for admitted counts. No renormalization
conceals lost probability.

`validation/reference.py` instead integrates the expanded Beta density with
200-digit Decimal Horner evaluation and computes quantiles in 120 bisections.
Its predictive probabilities are exact Python fractions from factorials.
`validation/FREEZE.json` commits source, oracle and input identities before the
first application check or execution. The validator requires quantile error
≤10^-7, an independently recomputed CDF residual ≤10^-8 at each *printed*
limit, and predictive agreement ≤10^-9. Mean, variance and P(p>0.5) have
10^-10 absolute tolerance. It additionally checks distribution normalization,
predictive expectation, all-success/failure and complementary symmetry,
batch order/regrouping invariance and equivalent whitespace.

From the repository root with the documented toolchain setup and a compiler
built from the qualified checkout:

```sh
omega update --project samples/cli/independent/germination_probability_inference --target linux_x86_64
# Review exact pending authority decisions before resuming, when required.
omega run --both --keep samples/cli/independent/germination_probability_inference/main.omg < samples/cli/independent/germination_probability_inference/input.txt
python3 samples/cli/independent/germination_probability_inference/validation/validate.py --omega /absolute/path/to/omega --route all
```

The validator compiles and runs the actual native product with each finite
input. It also links a private output observer against that supplied compiler's
exact fingerprinted Rust library closure using `tools/link_terminal_sponsor.py`.
The observer prepares the complete admitted project, retains its canonical
Terminal artifact and executes it with the report's exact runtime provisions,
explicit containing-domain exit delegation, fuel and wall-clock allowances.
Actual stdout, stderr, status and numerical values are captured per case.
`omega run --both` exit agreement alone does not establish numerical agreement.
Declines, mismatches, wrong source rejection and exhausted allowances fail.

`controls/predictive_bin` is a separately discoverable, package-free
source-invalid project: access at index 33 in `[f64;33]` must reject with
`cannot prove index \`33\` is within length 33`. It never enters native execution.
The application explicitly binds Linux x86-64, Windows x86-64 and macOS ARM64
program entries. Host qualification is reported in `VALIDATION.md`; other
hosts are not claimed by declaration alone. The only dependency is bundled std
console. Existing Beta occurrences were text labels, not assay inference; no
matching germination or beta-binomial workflow was present on fetched main.

The trailing-whitespace scanner deliberately uses a recursive blocking terminal
call. It exposed a source-resolution defect: free-machine self-recursion was
reclassified by leaf spelling as a state jump, discarding its `block` marker.
The general repair resolves machine calls before identity-based tail
normalization, retains exact markers and machine-invocation identity on
named tail edges, and restricts parenthesized state recovery to unmarked sibling
states. Canonical contracts are `wiki/spec/language/state_contracts.md`
(terminal calls) and `wiki/spec/language/effects.md` (exact acknowledgements).
The focused recursive-call corpus controls also cover parenthesized and
qualified call forms, missing and redundant markers.
