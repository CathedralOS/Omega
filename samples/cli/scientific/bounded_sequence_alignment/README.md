# Bounded biological local alignment

The Omega application reads two biological sequences from stdin, computes a
scored local alignment with a private nested 17×17 score/direction arena, follows its
traceback, and prints score, coordinates, and both aligned strings. Nothing is
imported from another sample or an in-progress numerical library.

Each sequence contains 1–16 ASCII A/C/G/T/N bases. Lowercase is normalized.
Lines use LF; the second may end at EOF. CR, whitespace, FASTA syntax and other
bytes are rejected. A seventeenth base is rejected before storage. The first
error exits 64 with a specific diagnostic; unavailable input is distinguished
from an explicitly empty line. Input after the second line is outside this
single-pair protocol and is not read. Internal invariant failure exits 70.

A canonical match scores +3, a mismatch −2, and each gap column −2. N scores −1
against every base, including N. This is local alignment with linear gaps,
not an affine-gap or probabilistic model. Cells start at zero. Strict updates
prefer diagonal, then up (deletion), then left (insertion). Only a strictly
larger global score replaces the endpoint, choosing the first row-major maximum.
Coordinates are zero-based half-open. No positive match emits score zero,
empty alignments and 0:0 coordinates. Maximum score is 48; all numeric results
are integral and the frozen tolerance is exactly zero. Wrapping scratch indices
are bounded by explicit guards before every array access; input capacities make
all actual arithmetic fit signed 32 bits.

`main.omg` owns input, computation and output dispatch. `input/sequence.omg`
parses bytes, `alignment/local.omg` owns the entire matrix and traceback,
`output/report.omg` renders results. Each traceback is also the prefix window of its 32-byte line whose length is the wrapping column count. `build.omg` binds the real entrypoint to
existing std providers on Linux x86_64/arm64, Windows x86_64 and macOS arm64.
The complete separate project in `invalid/` deliberately reads element 16 of
an array with capacity 16 and must be rejected by checking.

The existing `samples/cli/text/substring_search` scans for literal matching
bytes and returns an index. This workflow adds score optimization, mismatch and
gap competition, ambiguity, stable ties, coordinate recovery and traceback.
`maze_flood` computes BFS distances and supplies no alignment operation.

From the repository root, use the current compiler binary as `omega`:

```sh
omega update --project samples/cli/independent/bounded_sequence_alignment --target linux_x86_64
# Review the generated package proposal; accept only the documented decisions.
omega update --resume --project samples/cli/independent/bounded_sequence_alignment
omega --check samples/cli/independent/bounded_sequence_alignment/main.omg
omega samples/cli/independent/bounded_sequence_alignment/main.omg
omega run --both --keep samples/cli/independent/bounded_sequence_alignment/main.omg < samples/cli/independent/bounded_sequence_alignment/data/gap.stdin
python3 samples/cli/independent/bounded_sequence_alignment/reference.py --validate PATH_TO_NATIVE_BINARY OUTPUT_DIRECTORY
omega --check samples/cli/independent/bounded_sequence_alignment/invalid/out_of_bounds.omg
```

`data/oracle.json`, `data/expected.json`, `data/short_expected.json`, and the gap input/output were frozen
before Omega execution. `reference.py --freeze` reproduces those exact outputs
and compares 1,521 short sequence pairs over A/C/N to an independent exhaustive
path enumerator. The enumerator explores every possible alignment path from
every start without a score matrix. Native validation checks the 13 frozen
variants, all 144 pairs of lengths 1–2 over A/C/N, 11 malformed inputs, and final
EOF without LF. It compares complete stdout and exit values, verifies each
reported interval spells the ungapped input substring, and independently sums
all traceback column scores. The variants cover both gap directions, internal
mismatch, N ambiguity, competing maxima, earliest ties, lowercase, and capacity.

Terminal comparison must report actual agreement, not DECLINED. The CLI compares
termination only; separate native output checks remain necessary. Generated
products and execution evidence belong in ignored `build/`. Reference Python is
validation tooling; the Omega application performs all runtime work.
