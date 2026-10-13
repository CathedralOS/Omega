# D-optimal calibration design

This stdin application chooses a subset of candidate observations for a linear
or quadratic polynomial calibration. It maximizes `det(sum precision*f*f^T)`,
where `f=[1,x]` or `[1,x,x*x]`. Precision is inverse observation variance.
Selection uses exact integers and selects the lexicographically smallest tuple
of zero-based input indices on ties. Repeated settings remain separate candidate
observations, and every input index can be selected once.

Supply one finite whitespace-separated record:

```
quadratic 5 3 -10 2 -4 9 0 1 3 7 10 3 -10 0 10
```

The fields are mode (`linear` or `quadratic`), candidate count, subset size,
`count` pairs of integer setting and precision, and exactly three prediction
settings. Candidate count is 3–12; subset size is 1–6 and at most the candidate
count. Settings and predictions are in [-10,10]; precision is in [1,10].
ASCII space, tab, CR and LF delimit tokens. Integers have one or two digits with
an optional minus. The stream is bounded to 512 bytes; extra values, missing
values, invalid modes, signs, characters, counts and precisions are refused with
`invalid input` and exit 2. EOF terminates the record. Supply a finite file or
pipe; the comparison CLI snapshots stdin through EOF.

Output gives selected input indices, row-major information matrix rows, exact
determinant and status. An identifiable result also gives covariance rows and
three prediction variances `f^T covariance f`. Every rational is an exact
`numerator/determinant`, with signed zero-padded 18-digit integers. These are
numerical values without floating rounding; denominators are positive. There is
no unknown noise-scale estimate or additional observation noise in a prediction
variance. Exit 0 means identifiable. If every subset is singular (including a
subset too small for the requested degree), output still gives the lexicographic
selection, information and zero determinant, then `status non-identifiable` and
exit 3. Covariance and prediction variance are undefined and omitted.

The implementation enumerates candidate bit masks, excludes masks outside the
active candidate count and filters on cardinality. An integer reversed-bit key
orders ties by their input tuples. Moment sums and signed cofactors fit `i64`
throughout the bounded domain, including negative settings. No determinant,
selection, inverse or variance decision depends on floating arithmetic.

From the repository root, with an optimized compiler of recorded provenance:

```
omega update --project samples/cli/independent/d_optimal_calibration_design --target linux_x86_64
# Inspect any pending review; accept only the authorized exact inventory, then resume.
omega update --resume --project samples/cli/independent/d_optimal_calibration_design
omega run --both --keep samples/cli/independent/d_optimal_calibration_design/main.omg < samples/cli/independent/d_optimal_calibration_design/input.txt
python samples/cli/independent/d_optimal_calibration_design/validate.py --omega /absolute/path/to/omega
```

`reference.py` freezes independent Python-integer combination enumeration,
permutation-expanded determinants and `Fraction` Gauss–Jordan inverses before
sample execution. `validate.py` checks the frozen digest, exact indices, all
information entries and determinant, and exact rational covariance/variance
values (stronger than 1e-8 absolute/relative accuracy). It separately validates
native stdout and the retained `terminal.stdout` produced by `run --both --keep`;
exit agreement alone is insufficient. The 46 frozen inputs cover unequal
precision, nontrivial ties, repeated settings, singular candidate populations,
minimum and maximum bounds, asymmetric predictions, and runtime refusals.
`controls/candidate_bound` must reject index 12 of a 12-element source array for
its authored bounds diagnostic. Native executables can be supplied with
`--native` for inexpensive repeated numerical validation.

For a fast native iteration after retaining a produced executable, run:

```
python samples/cli/independent/d_optimal_calibration_design/validate.py --omega /absolute/path/to/omega --native /absolute/path/to/omega-program --native-only
```

Native-only and native/Terminal runs retain separate result directories. Full
qualification still requires captured Terminal values; the native iteration path
does not replace that gate.

Repeated full requalification can use one admitted Terminal artifact. This optional
linker currently requires mbx build receipts and its output layout; the public CLI
validation route above also works with a compiler built by ordinary Cargo:

```
python tools/link_terminal_sponsor.py --omega /absolute/path/to/omega --source samples/cli/independent/d_optimal_calibration_design/validation/terminal_capture.rs --output samples/cli/independent/d_optimal_calibration_design/build/terminal-sponsor
python samples/cli/independent/d_optimal_calibration_design/validate.py --omega /absolute/path/to/omega --native /absolute/path/to/omega-program --sponsor samples/cli/independent/d_optimal_calibration_design/build/terminal-sponsor
```

Retain `omega-program` from the public CLI's `--keep` directory. The sponsor
links the verified compiler's exact dependency units, performs normal accepted
project preparation, compiles the complete Omega application, and executes its
original canonical Terminal artifact and runtime provisions. It never accepts
package decisions or interprets source trees. Each run retains actual streams,
status and exit; unsupported execution, exhausted fuel and timeouts fail. The
validator verifies compiler, sponsor, source and library hashes, then checks
both actual native and Terminal numerical values against the frozen oracle.
