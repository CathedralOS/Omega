# Pedigree allele constraints

A complete Omega console application for a declared synthetic single-locus
scientific model. It enumerates all bounded genotype assignments, emits possible
genotypes and exact conditional probabilities, and gives an irreducible
observation contradiction witness. It is not a clinical inference model.

Model: alleles A/B, genotypes AA/AB/BB, independent fair founder chromosome
copies, fair transmission of each parental chromosome copy, no mutation,
no observation error. Founder genotype weights are 1,2,1 out of four. A child's
weight is the number of its parents' four chromosome choices producing its
genotype. Shared ancestry uses the same assigned ancestor throughout the graph;
it does not replace relatives with independent draws.

Supply EOF-terminated whitespace-separated **single decimal digits**: first the
number of people (1..6), then one row `father mother observation` per person.
IDs are 1..6 in row order. Founders have both parents zero. Other people have
two distinct earlier parent IDs. Observation mask bits 1,2,4 admit AA,AB,BB;
3 admits AA or AB, 6 admits AB or BB, 7 is unknown. Cycles, missing parents,
zero masks, extra/truncated tokens, multiple-digit tokens and non-digit input
are rejected with `INVALID INPUT`, exit 64. CR/LF, spaces and tabs are accepted.

```text
3
0 0 3
0 0 3
1 2 4
```

The report uses `SOLUTIONS n`, `EVIDENCE numerator/4^people`, then
`POSSIBLE person genotype numerator/evidence_mass`. Only positive probability
genotypes appear. Fractions are exact and deliberately unreduced; tolerance is
zero. Exit 0 means consistent; exit 2 means impossible. An impossible report
lists `CORE person` observation IDs whose conjunction with all inheritance
rules is unsatisfiable. Greedy deletion in ascending ID order makes that core
irreducible, not necessarily minimum cardinality. Every retained observation
becomes satisfiable if individually removed from the final core.

`domain/model.omg` defines validation and the inheritance kernel,
`domain/input.omg` owns byte parsing, `domain/solver.omg` owns exhaustive inference
and contradiction deletion, and `domain/report.omg` renders computed results.
Only the stable std Console provider and core binding package are used. There
are no worker, numerical-library, Python runtime or shared-export dependencies.
At six people, enumeration visits 729 genotype vectors; core extraction requires
at most eight such passes. All weights and marginal sums fit in 4096. Explicit
Wrapping storage is never reached outside those established mathematical bounds.

Existing sample comparison before authoring: `algorithms/maze_flood` computes
BFS distance on a fixed grid; `algorithms/horner_eval` computes one polynomial;
`interpreters/stack_calculator` operates a fixed stack. These exercise array or
state mechanics, but do not perform weighted inheritance enumeration, share
ancestor assignments, condition ambiguous observations, or extract contradiction
cores. A repository search found no existing allele/pedigree workflow.

From repository root, with a compiler built from the recorded base:

```sh
omega --check --target linux_x86_64 samples/cli/independent/pedigree_allele_constraints/main.omg
omega update --project samples/cli/independent/pedigree_allele_constraints --target linux_x86_64
# Read the produced review, decide its individual requests, then resume if authorized.
omega update --resume --project samples/cli/independent/pedigree_allele_constraints
omega samples/cli/independent/pedigree_allele_constraints/main.omg
python3 samples/cli/independent/pedigree_allele_constraints/validation/verify.py /absolute/path/to/compiled/application
omega run --both samples/cli/independent/pedigree_allele_constraints/main.omg < samples/cli/independent/pedigree_allele_constraints/fixtures/shared_ancestry.stdin
omega --check --target linux_x86_64 samples/cli/independent/pedigree_allele_constraints/controls/bounds/main.omg
```

The last command must reject specifically for an unproved parent array index
(`cannot prove index`), not for parsing, packages, entry binding or another rule.
`controls/bounds` is an independent complete intended-invalid Omega project;
it deliberately omits a dynamic parent-ID guard. See the language's indexed
array bounds contract in `wiki/spec/language/numeric_values.md#collection-bounds` and the existing
`tests/omega/fail/collections/computed_selector_read_requires_bounds` control.

The independent Python oracle enumerates 4^people chromosome-choice histories,
then collapses them to genotype assignments. It does not use the application's
3^people algorithm or inheritance formula. `validation/FROZEN.sha256` was written
before any Omega check or execution. Do not regenerate expectations from an
application run. The 399 exhaustive tiny observation combinations and 23 named fixtures cover unknown/ambiguous founders, tiny trios, impossible
inheritance, shared ancestry, six people, two contradictions and malformed input.
`validation/verify.py` requires full stdout byte equality, exact exit status and
empty stderr, and records every case in JSON.

Validation checkpoint: package preparation checked both source packages on compiler
base `1ade44fe20d0979707c383846a4a25a40527209f`, but final `--check`
stops at program-entry admission because the required package acceptance was
rejected by automatic approval review. The 286 pending decisions include the
repository std package's external supplies and filesystem/process capabilities,
plus this application's console input/output/termination permissions. No denied
acceptance was retried. Application native and Terminal runs have **not occurred**;
the frozen 422-case oracle is pending execution, and landing is pending approval
and runtime validation. The bounds control rejects for its intended rule.

The bounded parser repair has two checked source controls and five intended
rejections. Its std-free blocking control independently passes native and
Terminal with exit 0 and empty streams. Combined suspension acknowledges correctly
during checking; the separate complete `validation/reproductions/terminal_suspend_markers`
project exposes an unsupported embedded Terminal signature on this compiler.
That refusal is retained as evidence, not treated as application execution.
