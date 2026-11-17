# Token and syntax representation ownership

The Rust token and syntax representations now start at `token_stream.rs` and
`syntax_trees.rs`, with related records in semantic folders and test bodies in
separate files. Parser-owned span construction now uses foundation's checked
`HandleSpan` operations. Tree copying and identifier-path joining belong to the
syntax representation, and eight table-access wrappers are gone.

This cleanup does not establish a speedup. The full authored parser workload
was 2.16% slower in the measured run; the other two workloads were inconclusive.
The change removes a temporary vector when copying expression handles between
tables, but also adds span-invariant checks where the old parser builder did
unchecked arithmetic. The experiment does not isolate those effects.

## Measurement

Baseline source: `c216c7b88d3eb1398151527dab08c912d36d0df9`.
Candidate: the accompanying representation-ownership diff, without the rejected
inline attributes described below. Token layouts and the timing harness are
unchanged.

Host: Windows x86-64, Intel Core i9-12900K, Rust nightly-2026-09-04, mbx 1.29.2.
The benchmark processes ran at Idle priority on logical CPU 8. This session ran
no builds during timing; the shared workstation was not otherwise isolated.

Build both revisions with
`mbx bench -p tokens-to-syntax-trees --bench parser_timing --no-run`, preserving
each executable. The existing parser harness lexes retained sources before the
timer, measures parsing plus tree construction/destruction, and checks token
fingerprints and tree snapshots outside the timer. `omega-bench compare` used
one warmup and 16 alternating AB/BA pairs, with matching work/checksums and a
30-second process timeout. Each reported batch targeted roughly 250 ms.

| Input | Repetitions per batch | Paired median time change | Bootstrap 95% interval |
| --- | ---: | ---: | ---: |
| All six authored files, 151,835 bytes | 21 | +2.16% | +1.12% to +3.99% |
| Standalone, 764 bytes | 76,220 | -0.87% | -3.26% to +2.49% |
| Float operations and numeric conversion, 80,087 bytes | 39 | +1.85% | -2.42% to +3.60% |

Positive values mean slower. These are parser measurements on this host, not
whole-compiler or native-program results.

One follow-up compared the candidate with and without `#[inline]` on the two
span operations. Its paired median was -13.85%, but baseline batches ranged
from 71.85 to 408.75 ns/unit and candidate batches from 71.16 to 566.98 ns/unit.
That run was too variable to attribute the difference to the attributes. Both
attributes were removed; no optimization claim depends on that run.

## Reproduction evidence

The local evidence archive is
`C:/Users/zacha/.codex/artifacts/rust-representations-2026-11-16/`.
It contains all pair reports, plans, frozen inputs, source hashes, executables,
the final patch, validation logs and publication identities. Plans retain their
original worktree paths; point them at the archived inputs and executables when
replaying elsewhere.

Executable SHA-256:

- Baseline: `28588500f1d44c9f5e1e32aac44e345361ff93378f80bd054f0ea94764ed4658`.
- Selected candidate: `8b513f82b6c916205fdda0f78693c826a5a7fb80a03eccf5360c84ac0fcd8d8f`.

The six frozen sources are `samples/cli/basics/print_number/main.omg`,
`samples/cli/basics/standalone/main.omg`, `source/library/core/math.omg`,
`source/library/core/compact_binary.omg`, `source/library/core/float_operations.omg`
and `source/library/core/numeric_conversion.omg`. Their exact hashes and byte
lengths are in `inputs.json`; subsequent edits to those authored files are not
part of this measurement.

## Validation

The scoped test runs cover 522 passing tests across arena, tokens, syntax trees,
lexer, parser and source compiler. Existing test names and source literals were
preserved when moving test bodies. Four new tests exercise contiguous-span
extension, and one checks retained identifier-path spelling and source spans.
Clippy passed for the four changed crates with all targets and warnings denied;
formatting, repository rules and the Psi/Omega dependency boundary also passed.
