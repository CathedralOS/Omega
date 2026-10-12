# Two-group differential expression

A bounded two-group count-data application with six biological replicates and
8 genes. Input is six group labels (0 = reference, 1 = contrast), followed by
48 integer counts in gene-major order, separated by whitespace and terminated
by EOF. Each group needs at least two replicates. Counts must be 0–1000; the
entire input is at most 599 bytes. Syntax errors exit 64; invalid metadata or
count ranges exit 65. No gene positive in all samples prevents median-ratio
normalization and exits 66. Runtime count and metadata errors do not disappear
as excluded statistical rows.

Size factors use all genes positive in every sample, before testing filters:
exp(median(log(count) - mean(log(count)))). The median for even cardinality is
the average of the two middle log ratios. Factors are not rescaled afterward.
A gene is excluded in this order: `all_zero`, `low_total_count` (sum < 10),
`group_zero_boundary` (one group's normalized mean is zero). Boundary cases
have no finite unpenalized log fold-change; no pseudocount is silently added.

This is deliberately a narrower statistical model than full DESeq2. Counts
are independent NB observations with mean s[j]*lambda[group[j]] and variance
mu + alpha*mu². Alpha is an unshrunk residual moment estimate, clipped to
[1e-6,10]. With m[j] the normalized group mean, the estimator is
sum((y[j]/s[j]-m[j])²-m[j]/s[j]) / ((4/6)*sum(m[j]²)). The four residual degrees
of freedom reflect six observations and two fitted group means. It is not a
Cox-Reid adjusted maximum likelihood estimate or empirical Bayes dispersion.
The moment formula is an explicitly defined illustrative estimator, not an
unbiasedness claim for unequal size factors or finite-sample calibration.

Given that dispersion, each group mean maximizes its offset NB likelihood.
Sixty bisections on [0,1e6] solve sum((y-mu)/(1+alpha*mu))=0. The size-factor
bounds induced by admitted counts put the solution below 1e6. Standard errors
use expected Fisher information sum(mu/(1+alpha*mu)) in each group; log2 fold
change is log2(lambda[1]/lambda[0]), SE is sqrt(1/I0+1/I1)/ln(2). Two-sided
normal Wald p-values feed stable-rank Benjamini-Hochberg reverse minima over
only tested genes. No dispersion trend, prior, fold-change shrinkage, outlier
replacement, Cook's filtering, independent filtering, batch covariate or
finite-sample exact test is implemented. No equivalence to full DESeq2 or
biological false-discovery calibration is claimed.

Output first has six `factor,index,value` rows. Each gene then reports
`gene,index,reason`, or `gene,index,tested,baseMean,dispersion,mean0,mean1,
log2FoldChange,lfcSE,logLikelihood,pvalue,qvalue`. Numbers have nine significant
decimal digits. The NB log PMF uses a private shifted Stirling log-gamma evaluator:
shift positive arguments to at least 12, then retain terms through x^-11.
The next asymptotic term at 12 is below 7e-17; double-precision cancellation
in differences at dispersion 1e-6 dominates the practical likelihood error. Private logarithm and exponential range-reduced series
are checked against independent library implementations. Normal-tail evaluation
uses Abramowitz-Stegun 7.1.26 with absolute error below 1.5e-7.
For |z| > 12 it returns zero (the exact tail is below 4e-33), keeping
the exponential evaluator inside [-72,0] and preventing overflow. BH can amplify p-value approximation error. Validation of the frozen fixtures allows
4e-7 absolute error for p/q, 2e-5 for log likelihood and 2e-7 for other columns,
plus 2e-7 relative tolerance (factors use 2e-8).

## Independent references

`validation/oracle.json` was frozen from Python 3.12, NumPy and SciPy 1.17.0
before executing the Omega application. `reference.py` uses `brentq`, `gammaln`
and `ndtr`; it does not copy the application's bisection or shifted Stirling evaluator,
series functions or normal-tail approximation. Its hash is in `oracle.sha256`.

`validation/deseq2.R` requires DESeq2 **1.46.0**, installed from Debian trixie's
`r-bioc-deseq2` package. It independently calculates normalization and moment
dispersion, filters the same genes, supplies these to `nbinomWaldTest`, and uses
`betaPrior=FALSE`, `minmu=1e-6`, `betaTol=1e-10`, `maxit=1000`, `useOptim=TRUE`.
`results` uses contrast group 1 / 0, `independentFiltering=FALSE` and
`cooksCutoff=FALSE`; `lfcShrink` and `DESeq` are never called. Frozen CSVs,
factors and `session.txt` retain the independent execution. `check_deseq2.py`
compares normalization, dispersion, base mean, fitted group means, NB log
likelihood, LFC, SE, p and BH q to the
SciPy oracle with 2e-5 relative / 2e-6 absolute tolerance. The small discrepancy
reflects DESeq2's internal numerical regularization. See the official
[DESeq2 vignette](https://bioconductor.org/packages/release/bioc/vignettes/DESeq2/inst/doc/DESeq2.html)
for the full pipeline and its different dispersion and filtering defaults.

Fixtures exercise unequal library sizes, overdispersion, unequal group sizes,
group swaps, sample permutation, ties, all-zero/low-count/group-zero exclusions,
absence of normalization genes, and invalid labels, replicate counts, counts,
noninteger values, missing and extra fields. `controls/intended_invalid` pins
an independently rejected out-of-bounds array access.

## Run and validate

From repository root, with a compiler built from the recorded candidate:

```sh
omega update --project samples/cli/independent/differential_expression --target linux_x86_64
# Inspect the exact generated source and capability review, then resume it.
omega run --keep samples/cli/independent/differential_expression/main.omg < samples/cli/independent/differential_expression/data/realistic.stdin
python -B samples/cli/independent/differential_expression/validation/run.py --executable /retained/build/path/omega-program
python -B samples/cli/independent/differential_expression/validation/check_deseq2.py
python -B samples/cli/independent/differential_expression/validation/check_observed.py
```

The private Rust `validation/terminal-driver` uses the ordinary package manager
and retained accepted Terminal artifact, runs each fixture with the Console
provider, and saves actual stdout/stderr and exit status. It has a separate
compiler identity, so its exact package proposal is reviewed and resumed through
its `--review` / `--resume-review` modes. No identity override, trust bypass,
source interpreter or approval bypass is used. Its native and Terminal outputs
are checked against the same frozen numerical oracle. Receipts live in ignored
`build/validation/`; committed evidence summarizes observed runs and identities.
After observing Terminal, the driver publishes that same retained native product
through the ordinary product API and prints its executable path. Pass that path
to `validation/run.py --executable` to check all native fixtures from the same
prepared compilation.

The retained build path is printed by `run --keep`; reuse that unchanged native
application for all fixture inputs. [Qualification evidence](validation/EVIDENCE.md)
and [actual output receipt](validation/observed.json) are committed.
