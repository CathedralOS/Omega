# Gene set enrichment audit

A complete bounded scientific sample, using synthetic feature IDs A–J. The
user supplies an annotation universe, selected features and three annotation
sets through stdin. This is an audit of a declared sampling model, not a
biological inference tool.

The null model draws n distinct features uniformly without replacement from
N supplied universe members. Each annotation contains K members; its one-sided
p-value is P(X >= observed overlap), computed by exact hypergeometric counts.
Three stable ascending p-value ranks feed Benjamini–Hochberg correction: take
the reverse cumulative minimum of min(1, 3*p/rank). Overlapping annotations are
allowed. The sample reports the numerical adjustment and makes no general FDR
guarantee for arbitrary dependence among annotations.

Five newline-terminated rows specify universe, query, set 1, set 2, set 3.
Each row has zero to 63 A–J bytes, followed by LF. The universe must be nonempty
and contain no duplicates. Query and annotation duplicates collapse to one
member and their duplicate counts are reported. Every member must occur in the
universe. Empty query/annotations are valid. EOF must immediately follow row 5.
Invalid characters, long rows, missing LF/rows, trailing input and invalid
universes exit 65 with exactly `INVALID_INPUT`. Valid inputs exit 0.

Output contains N/n, duplicate counts, original annotation identity, K, overlap
count, exact raw p and adjusted q fractions, stable rank, sorted annotation
members and sorted overlap provenance. Fractions need not be reduced. Zero
numerical tolerance applies: validation compares rational values exactly.
With N <= 10, choose(N,n) <= 252; tail counts <= 252; BH numerators/denominators
<= 756. Fraction comparison products <= 571536. Set masks <= 1023.
Private Wrapping arithmetic stays within those bounds for admitted input.
The multiplicative binomial update has intermediate products at most 2520;
its explicit nonzero divisor guard also defines safe behavior outside the
application admission.

For the overlapping fixture the exact (p,q) pairs are (1/30,1/10),
(1/3,1/2), and (1,1). The reversed-rank fixture reports ranks 3,2,1.
The equal-p fixture reports stable ranks 1,2 and gives both tied sets q=1/10,
which tests the reverse-minimum step rather than independent p*3/rank scaling.

`domain/sets.omg` owns bounded parsing and set storage; `domain/exact.omg` owns
integer combinatorics and fractions; `domain/report.omg` owns dynamic formatting.
`main.omg` owns admission, analysis, stable ranks and reporting. There are no
shared numerics imports or changes. The existing std Console provider and exact
Linux program-entry contract are the only runtime facilities.

## Reference and validation

Run from the repository root with the repaired candidate compiler. The recorded
base compiler reproduces the structural-copy custody gap. The verified repaired
binary SHA-256 is `caf8a6144f521a34c9867a51d43ed8cce7826fd97581d76a196ab27074821cda`:

```sh
python3 samples/cli/independent/gene_set_enrichment_audit/reference.py
python3 samples/cli/independent/gene_set_enrichment_audit/validate.py --omega /absolute/path/to/omega --route check
python3 samples/cli/independent/gene_set_enrichment_audit/validate.py --omega /absolute/path/to/omega --route native
python3 samples/cli/independent/gene_set_enrichment_audit/validate.py --omega /absolute/path/to/omega --route terminal
python3 samples/cli/independent/gene_set_enrichment_audit/validate.py --omega /absolute/path/to/omega --route invalid
```

The oracle was frozen before Omega execution. `reference.py` enumerates every
possible selected subset, independently of the application's binomial formula.
Six valid cases cover overlap, duplicates, reversed ranks, ties, empty query,
full query and sparse universes. Eight invalid runtime fixtures cover admission
and parser limits. `fixtures/expected.json` and `oracle.sha256` are the frozen
answers. Do not regenerate them from program output. The runner retains actual
stdout, stderr, command and exit receipts in ignored `build/<route>/`.
Terminal route acceptance requires explicit native/Terminal exit agreement;
a decline is not a pass. The current CLI compares process exits only. Native
stdout is checked against the complete exact oracle on both execution routes;
this CLI does not expose interpreted stdout for an independent output check.
Only one Omega execution runs at a time.

The separate complete `controls/intended_invalid` project deliberately reads
index 3 of a three-element array. Its exact documented Slice::index requires
contract must be structurally false; a parser/package/type error is not a pass.

## Existing sample comparison

`algorithms/insertion_sort` demonstrates fixed hardcoded compare/swap ordering;
this sample ranks runtime probabilities with ties and applies reverse minima.
`algorithms/longest_run` scans a fixed byte fixture; this sample admits bounded
user input, collapses repeated identifiers and verifies universe membership.
`text/parse_int` parses digits; this protocol parses provenance-bearing sets.
None computes an exact sampling distribution or multiplicity adjustment.

## Package review and receipts

Prepare the final source with `omega update --project
samples/cli/independent/gene_set_enrichment_audit --target linux_x86_64`.
Inspect the generated package review, decide each capability row explicitly,
and use `omega update --project samples/cli/independent/gene_set_enrichment_audit
--resume`. Freeze all package source before preparation and acceptance.
A previous proposal became stale after a documentation correction; it was
discarded before fresh review. Source identity rejection is recoverable; an
actual authorization denial must be respected. Exact commands and validation
receipts belong in the review bundle, separately from the frozen source.
