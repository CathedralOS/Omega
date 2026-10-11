# Variant record normalization (NEW061)

This is a complete Omega console application for a declared **synthetic,
individual haploid segment** model. It has no clinical interpretation, genomic
assembly lookup, external datasets, numerics imports, or worker dependencies.
All sequences and calculations are owned by this sample. Two independently
supplied contigs hold 1–8 A/C/G/T bases. Coordinates are zero-based boundaries;
reference alleles and alternate alleles each hold at most four bases. A batch
holds at most 512 input lines and 32 distinct canonical variants.

The input stream consists of LF-delimited reference declarations followed by
variant records:

```text
R0:AAAA
R1:ACACAC
V0:3:A:-:0
V0:1:A:-:1
V1:4:AC:-:6
```

`R<contig>:<reference>` supplies a complete synthetic segment. Contig ids are
0 or 1, and each may be declared once, before any syntactically valid variant.
`V<contig>:<position>:<REF>:<ALT>:<source>` replaces the declared reference
allele at a single-digit coordinate (0–8). `-` or an empty field denotes an
empty allele. Source ids are 0–7. CRLF, whitespace, ambiguous bases and other
trailing data are rejected. A final unterminated line is accepted at EOF.
Malformed input receives a numbered rejection and does not mutate the ledger.
A line exceeding 64 bytes or a 513th input line ends the batch with `FATAL`.

For a validated edit the application constructs the whole edited segment. It
enumerates all bounded replacement coordinates and checks prefix/suffix
agreement against that segment. The canonical edit minimizes
`(removed length + inserted length, position, removed length)`. The inserted
sequence is fixed by those coordinates. This permits unanchored zero-length
alleles at either contig boundary; it is deliberately a segment edit model,
not a claim to implement VCF's anchored representation convention. Equivalent
repeat indels and redundant flanking context collapse to one edit. No-change
records are rejected separately.

Canonical rows retain first-seen order and merge source bitmasks by bitwise
union. Different canonical rows conflict when their closed reference intervals
intersect on the same contig. Insertions occupy one boundary, including either
endpoint of a deletion. This conservative model reports conflicts without
asserting that the alleles cannot coexist in a biological sample.

Output is deterministic pipe-separated text:

- `CANON|contig|position|deleted_length|inserted|source_mask|action`, with the
  mask printed as an unsigned decimal integer,
  and action 0 for new, 1 for merged.
- `MERGE|index|contig|position|deleted_length|inserted|source_mask` reports final
  canonical records and all contributing sources.
- `CONFLICT|earlier_index|later_index` identifies every conflicting pair.
- `REJECT|line|reason`: 1 syntax, 2 missing/duplicate/late reference declaration,
  3 contig bounds, 4 reference mismatch, 5 unchanged haplotype, 6 ledger capacity.
- `SUMMARY|unique|merged_input_records|conflicting_pairs|rejections`.

Exit 0 means every record was accepted; exit 2 means a rejection or fatal
capacity limit occurred. Rejected records cannot contribute provenance.

## Project and independent expected results

`main.omg` owns input/dispatch and final reporting. `domain/input.omg` parses
bounded input, `sequence.omg` owns private storage, `normalization.omg` owns
haplotype construction and canonicalization, `ledger.omg` owns merges, and
`report.omg` formats output through the existing Console provider. `build.omg`
binds the exact entry to Linux x86-64 and uses the stable bundled std package.
The sample introduces no shared exports.

Semantic overlap was checked against `samples/cli/text/substring_search`,
`collections/generic_ring_buffer`, `algorithms/maze_flood`, and
`systems/account_ledger`. They provide examples of guarded reads, bounded
storage and reports. None normalizes sequence edits, independently enumerates
haplotype equivalence, or merges allele provenance/conflicts.

`validation/reference.py` independently enumerates actual inserted A/C/G/T
strings and applies edits to form a haplotype-to-canonical-edit map. It does not
use the Omega prefix/suffix algorithm or read compiler outputs.
`validation/freeze.py` authored and froze 41 fixtures before Omega execution,
including 714 valid input edits over every nonempty AC segment of length at
most 3, deletion widths at most 2 and inserted AC strings of length at most 2.
Other fixtures cover repeat indels, different contigs, conflicting alleles,
minimal substitutions, both contig endpoints, reference mismatches, invalid
runtime syntax, unchanged sequences, missing references, final EOF, line,
record and distinct-variant capacity. Comparison is exact bytes and exit;
all calculations are integer and numerical tolerance is **zero**.

Expected data were frozen at commits `0fc5a0fb52` and `e0470cf6aa`, before check,
native or Terminal execution. The second commit adds the stream capacity case.
`validation/oracle.json` records each expected exit and input/output SHA-256.
Never regenerate expected files from observed output. Independent review identified
LF-only segmentation and overlapping capacity precedence issues in the reference
implementation. The corrected oracle preserves all original 41 outcomes and the
original manifest hash. `validation/review_oracle.json` freezes eight supplemental
controls before runtime execution, including CR, VT, FF, NEL, Unicode separator,
NUL, and an overlong 513th line. The executable verifier compares all 49 cases.

## Commands

From the repository root, select a compiler built from this exact main base,
set `OMEGA_LIBRARY_ROOT` to this checkout's `source/library`, and use one
compiler execution at a time. `OMEGA` below is the verified binary path:

```sh
sample=samples/cli/independent/variant_record_normalization
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
"$OMEGA" update --project "$sample"
# Inspect the generated review and accept only reviewed required decisions;
# then use the documented update --resume workflow. Do not retry a denial.
"$OMEGA" --check "$sample/main.omg"
"$OMEGA" "$sample/main.omg"
python3 -B "$sample/validation/verify.py" --executable "$sample/build/linux_x86_64/omega-program"
"$OMEGA" run --both --keep "$sample/main.omg" < "$sample/data/repeats.stdin"
"$OMEGA" --check "$sample/intended_invalid/main.omg"
```

The last command must reject specifically with `cannot prove index`; an
entry-binding, package-review, parse, cast or other rejection does not pass the
control. The intended-invalid project is a complete entry project with runtime
input, a helper taking an unconstrained dynamic index and a reachable indexed
read. Its build must receive its own ordinary package review if required.

`run --both` uses the produced Terminal artifact and finite input replay. The
current CLI compares termination; it does not expose an independent Terminal
stdout oracle. Record agreement, disagreement or explicit decline as observed,
and retain that limit separately from the exact native stream comparison.
Build/compiler/validation identities and actual route results are recorded in
`validation/RESULTS.md` when execution finishes. Generated output and logs live
under local `build/` and are included separately in the review bundle.
