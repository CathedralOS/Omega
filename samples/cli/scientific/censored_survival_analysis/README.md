# Censored survival analysis

Synthetic records only. Supply a record count (1–32), followed by `time event group`
triples separated by ASCII whitespace, then EOF. Time is a finite nonnegative
decimal up to 1000000 with at most seven whole digits and six fractional digits.
A whole digit is required before the decimal point. Event and group must each
be exactly 0 or 1; event 0 means censored. Missing records, extra tokens, malformed
numbers, invalid flags, out-of-range counts/times and input over 4096 bytes exit
2 with `ERROR invalid input`. No patient data or clinical interpretation.

```sh
omega update --project samples/cli/independent/censored_survival_analysis --target linux_x86_64
omega --check --target linux_x86_64 samples/cli/independent/censored_survival_analysis/main.omg
omega run --both samples/cli/independent/censored_survival_analysis/main.omg < samples/cli/independent/censored_survival_analysis/data/tied/input.txt
```

Read pending package decisions and resume the normal review. Source-risk
information never grants host authority. The explicit ProgramEntry binding and
private runtime ProcessExit delegation use the ordinary accepted package route.

Output has `TABLE -1` (pooled), `TABLE 0`, and `TABLE 1`. Each `ROW` contains time,
pre-event risk, events, censors, survival, Greenwood variance, and zero-survival
status (1 means survival is zero and variance uses its zero limiting value).
Tied censors remain in the event risk set. Censor-only times are retained.
`MEDIAN` contains reached status (1/0), first time survival is at most 0.5 (0
placeholder when unreached), and row count. An empty group has zero rows and an
unreached median; its survival is conceptually 1 before observation.
`LOGRANK` contains observed group-0 events, expected group-0 events, summed
hypergeometric variance, chi-square statistic, and estimable status (1/0).
Zero variance yields status 0 with statistic 0 as an explicitly undefined
placeholder. No p-value or asymptotic significance claim is made.

The median threshold uses an exact four-limb product fraction (bounded by 32!),
so rounding near one half cannot change its status. The solver rescans risk sets
at distinct times, so record order cannot affect counts. `validation/reference.py` freezes exact Fraction product-limit tables
and independently enumerates all equally likely event subsets at each event
time for log-rank expectation/variance. Frozen inputs, expectations and roster
are authenticated by `oracle-freeze.sha256`; do not regenerate to accommodate
execution results. Additional review challenges are frozen independently in
`extra-freeze.sha256`; `all_cases.txt` joins the 44 unique cases. Run `python -B validation/validate.py --native IMAGE --captures DIR`,
or `python -B validation/validate.py --terminal SPONSOR --captures DIR`. Both verify exact counts/statuses and
absolute numerical error at most 1e-9. The Terminal sponsor retains and executes
the accepted native product's exact Terminal artifact and runtime provisions,
and captures its actual stdout/stderr; exit parity alone is insufficient.

The separate `controls/intended_invalid` project must reject negative, past-end,
and unproved dynamic array access. It is never accepted or run. This source control complements
runtime input refusals. The bounded table count and guarded row states establish
all valid dynamic indexing obligations.

Qualification uses 25 valid cases and 19 runtime refusals, with 985 numerical
fields per execution route. Native and actual captured Terminal outputs meet
1e-9 absolute tolerance; the measured maximum is 6.4e-11. Exact row counts and
statuses are checked separately. The sample gate also discovers and verifies
the three source-invalid indexing diagnostics. See `validation/QUALIFICATION.md`
for producer and evidence provenance.
