# Body-bearing equality and scalar qualification

Equal f64-in-Marked carriers. The selected body returns false, the direct comparison is false, and the match arm misses with result 11.

Owning task: **OPERATOR-MACHINE-SUPPLY** in [TASKS.md](../../../../TASKS.md).
This row establishes declaration-owned binary and match equality, preservation
of the compared scalars' recorded domain memberships through checked-to-Terminal
lowering, and canonical Terminal artifact re-ingestion. The selected `==` body
returns `false` for two equal `1.0` carriers; `hold(direct)` requires that result
be false, and `choose` must take the wildcard with result 11. No caller guard
establishes the fact after the comparison.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [call_result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [match_join](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [const_name](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| another state | [other_state](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [other_machine](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| another target body | [target_body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The [root](main.omg) and every positive declare exit 0 and bind ProgramEntry
for windows_x86_64, linux_x86_64, macos_arm64 and linux_arm64. The local twin
explicitly erases Marked before comparison, selecting builtin equality; the
other twins change only the selected body's result to true. All reject the
same `hold` obligation, with `cannot prove requires contract for call hold`
and `direct == false`. Fixtures are byte-identical under
`tests/omega/{pass,fail}/facts/body_bearing_comparison_through_<place>_*`.

The same field production must retain qualifications at every producer, rather
than repairing one constructor spelling:

| Production | Positive and sensitive twin | Additional obligation |
| --- | --- | --- |
| default construction and stores | [default_fields](default_fields/main.omg), [twin](default_fields/invalid/main.omg) | zero initialization and subsequent literal/call-result replacements carry the receiving Marked field requirement |
| case payload | [case_fields](case_fields/main.omg), [twin](case_fields/invalid/main.omg) | payload construction and arrival retain both qualified scalar fields |
| ordered calls | [ordered_field_calls](ordered_field_calls/main.omg), [twin](ordered_field_calls/invalid/main.omg) | two mutable calls run exactly once each before their completed values receive Marked |
| saved field result | [saved_field_result](saved_field_result/main.omg), [twin](saved_field_result/invalid/main.omg) | a copied comparison result stays false after an alias overwrites the source field to true |
| stronger source | [stronger_fields](stronger_fields/main.omg), [twin](stronger_fields/invalid/main.omg) | sources retain the complete Marked & Extra set while fields require Marked; later calls consume both original memberships |

The [alias control](ordered_field_calls/invalid_alias/main.omg) reads the field
after it is overwritten through `&mut`; its old false result must not justify
`hold`. It is pinned as `body_bearing_comparison_overwritten_field_result_rejected`.
Each other production has matching pass/fail facts fixtures. Every positive
facts fixture also declares `expected_terminal_inspections.txt` with `main`,
so the corpus route checks source production, canonical encoding and decoding.

Establishment and soundness follow [domains](../../../../wiki/spec/language/domains.md),
[scalar qualifications](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#scalar-qualifications)
and [structural access](../../../../wiki/spec/terminal-psi/structural_access.md).
The checker admits predicate-free, route-free Marked; receiving lowering creates
a fresh qualified SSA value from the completed operand through the ordinary
qualification edge. It preserves stronger source memberships and cannot mint
predicate or issuer evidence, replace unrelated sets, replay effects or erase
an existing qualification implicitly. The zero/predicate, issuer and mutable
reference-erasure controls in `tests/omega/fail/domains/scalar_field_*` retain
their intended refusals.

Identity and composition follow [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).
A completed reference-free initializer installs its current value after its
operand effects. An earlier opaque write cannot invalidate a binding that did
not yet exist. Subsequent writes retire the current storage observation;
detached scalar snapshots keep their identity. Source checking, exact actual
substitution, every join/arrival and all target-scoped bodies remain checked.

## Observations

The frozen baseline at `41a6f1336117d8d6979ba24f82e2935ba802d83c` emits the
field artifact, then refuses it with `scalar field write lacks its required
domain identity`. In-memory execution alone missed that defect. The ordered
mutable-call production also refused the independent `hold` obligation after
an opaque earlier write had retired future local declarations.

At producer `d999ceec45bc92e27807d7bef1a736f358587090` (optimized dev/test,
Windows x86_64), pinned qualification observed all nine places, the root, five
other productions and the returned-Boolean rung run with native exit 0 and
Terminal exit 0. All sixteen source artifacts encode/decode; all sixteen exact
`program.oterm` products retained by `run --both --keep` independently decode.
Their native images, identity files and comparison streams are retained with
hashes. All sixteen negative controls reject their intended obligation.
The focused corpus checks 39 fixtures: 16 checked and 23 rejected, with zero
wrong-reason rejections, lost successes, fail fixtures accepted or unfulfilled
positive oracles. Inspection facts and newly added fixture rows are the intended
golden changes. Two other diagnostic differences already reproduce on the
fetched baseline and are excluded from the scoped golden record.

```text
omega run --both --keep samples/cli/facts/body_bearing_comparison/<place>/main.omg
omega --check samples/cli/facts/body_bearing_comparison/<place>/invalid/main.omg
omega inspect-terminal --machine main --output field.psi samples/cli/facts/body_bearing_comparison/field/main.omg
omega inspect-terminal --artifact field.psi
python tools/corpus_gate.py --runner <qualified-runner> --filter facts/body_bearing_comparison
```

The owning claim retains exact producer hashes, source/oracle identities,
qualification receipts and raw streams. Compile-only foreign-target checks are
separate from the observed Windows execution.

The historical Intel macOS x86_64 observations at `2ece6cb7889b` and
`1654d389500` used scratch host bindings and established in-memory agreement,
while field and state artifacts still failed decoding. They do not establish
execution on the four authored hosts. Current execution is measured only on
Windows x86_64; native execution on Linux/macOS ARM64 remains unmeasured here.

The [returned Boolean rung](returned_boolean/main.omg) and its
[twin](returned_boolean/invalid/main.omg) retain the independent requirement
that a body-wide constant joins all normal returns, states and target bodies.
This repair does not close OPERATOR-MACHINE-SUPPLY: supplied-callable result
constants and the task's other formation/operator acceptance remain on the board.
