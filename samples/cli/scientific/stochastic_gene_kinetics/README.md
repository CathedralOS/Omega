# Stochastic gene kinetics

This console sample runs Gillespie's continuous-time direct method with integer
populations. A one-byte command and LF selects a model or a refusal control.
The default model is immigration–death, with immigration propensity 2 and death
propensity X. The gene-expression model adds translation propensity X and
protein death propensity P/2. Transcription and mRNA death remain 2 and X.
Time is continuous, each wait is `-log(U)/sum(propensities)`, and a second
uniform selects a channel in transcription, translation, mRNA death, protein
death order. Events past time 2 are not applied. A zero total rate completes
without attempting a draw.

`0` and `1` print 1,024-replica immigration and gene ensembles. `2` and `8`
print their event trajectories (seed 100000). `7` repeats immigration with a
different seed (314159). `3` rejects a negative transcription rate, `9`
rejects a negative decay rate, and `4` rejects a negative signed initial
population. `5` begins at the admitted population cap and forces a birth to report overflow. `6` admits zero events
and reports exhaustion before a pending event. EOF and malformed commands
reject. The model's population cap is 100000 and its per-replica event cap is
100 (20 for traces). This is a bounded demonstration, not an unbounded
biological simulation.

The Park–Miller stream is explicit and seeded; the product fits in 64 bits.
The Omega logarithm range-reduces to [0.5,1), then evaluates 16 odd terms of
the atanh series. The independent Python reference uses `math.log`, not the
Omega logarithm. `data/frozen_oracle.json` and `data/FROZEN.sha256` were saved
before Omega execution. Exact seeded ensemble counters and per-event channel,
population and rounded time are checked against that reference. The reference
alone does not establish statistical correctness: the validator also checks
that mRNA's empirical mean, second raw moment, and four Poisson bins agree with
the analytic immigration–death law at time 2. The bin interval uses a four-bin
Hoeffding union bound at total false-rejection probability 1e-4 per ensemble.
First and second moments use six standard errors from the Poisson law.
Protein mean is checked against the exact two-stage expectation using six
empirical standard errors. These bands are diagnostics with stated sampling
assumptions; a different RNG seed is tested as an independent sample.

Run on Linux x86-64 after the package's exact review is accepted; set `OMEGA` to the current compiler binary:

```sh
"$OMEGA" update --project samples/cli/independent/stochastic_gene_kinetics --target linux_x86_64
"$OMEGA" --check --target linux_x86_64 samples/cli/independent/stochastic_gene_kinetics/main.omg
python3 samples/cli/independent/stochastic_gene_kinetics/validate.py --cli /absolute/path/to/omega
```

The validator builds once, repeats two seeds and runs the resulting native
binary for twelve inputs, then runs `omega run --both --keep` for each input. It preserves native and
Terminal stdout, stderr, exit codes and commands under the ignored sample
`build/validation` directory. The retained Terminal output is independently
checked against the frozen trajectory and ensemble values, as well as compared
byte-for-byte to native output.
Other targets have entry bindings but still require their own provider review
and native validation.
EOF is the default sample-gate command, with expected exit 2 and an authored
output fragment in `main.omg`.
