# Source-free table-field realization

This is a bounded increment toward source-free per-target compilation, not
completion of provider settlement, interpreter support, or Cathedral boot.

## Changed ownership

Source preparation retains the exact closed table storage application and
nominal field declaration in the provider row. Independent package review checks
that correspondence against the supplied declaration. Target realization reads
the row and canonical Terminal storage, validates a live non-atomic `addr` field
and a readable formal borrowing that exact record, and computes its target
offset through the common boundary-layout owner. The Source recipe adapter is
deleted. Borrowed layout demand uses the same scalar-extent completion as ABI
geometry; physical dependency discovery follows selected opaque carriers and
actual stored fields rather than their logical alternatives.

The runnable witness exposed two native gaps. AArch64 table calls now encode
full-u32 field offsets with independent decoding, exact memory/control effects,
and explicit scratch-register exclusions. Foreign operand replay consumes the
complete placement roster, excluding only the dispatch prefix from the callee's
wire signature. Vtable fields retain that prefix; table functions do not.

## Observed producer and commands

The fixed author base was `ecf27b2ce4992449c66607b64cdff5c23cc4e9ba`.
An actual conflict required integration of main `2b2f52bc1c7`; the tested
integrated source is `e81e92bdf28b2e8e5877d3d572ef352b93121b2d`.
No rebuild was triggered by subsequent main movement.

The corpus runner SHA-256 is
`a9aa0de531b6b0eef7169a400fb3007467588c9131be4bc4aa1088fcefad87b5`.
Its adjacent verified build-profile receipt records effective optimization
level 1. The existing integration executable SHA-256 is
`7923ca94522b7373fb7491ee730793645d3efc8a9c9ff8807f4a94d5cbd3af62`.
The pinned source bundle and raw reports are attached to
[PR evidence](https://github.com/CathedralOS/Omega/releases/tag/evidence/pr1436-source-e81e92bdf28b).

Focused commands used `tools/corpus_gate.py --runner <verified-runner>` with
the following exact filters; each recorded run also supplied `--record --json`.

| Filter | Mode | Actual result |
| --- | --- | --- |
| `pass/providers/table_leaf_dispatch_parameter_exit` | check | Checked, macOS ARM64, 3.328 s |
| Same table witness | `--native` | Native exit 0, 3.264 s |
| `fail/providers/table_dispatch_non_address_field` | check | Intended `must have type addr` refusal |
| `fail/providers/direct_foreign_wrong_table` | check | Intended `no runtime field` refusal |
| `fail/providers/table_leaf_without_dispatch_borrow` | check | Intended readable-borrow refusal |
| `pass/build/terminal_native_layout_catalog` | `--native` | Exit 0 and native-layout/Terminal-inspection controls match, 7.824 s |
| `pass/build/default_component_abi_extents` | `--both` | Native 0, Terminal 0 |
| `pass/build/terminal_private_callback_calling` | `--both` | Native 0, Terminal 0 |

The existing shared-target integration test passed in 96.76 s: one shared
publication, target-failure isolation, and native equality with corresponding
single-target outputs. An engineering-only Source-discard/reload probe passed
in 6.56 s. The newly added Rust probe and artifact-custody behavior test were
removed before landing to follow `tools/testing.md`'s fixture-based retention
policy; their raw e81 observations are historical evidence, not retained tests.
Retained focused unit observations were plan-preimage 10/10, physical storage
dependency queries 3/3, AArch64 encoding 2/2, and effects catalog 1/1.

## Limitations and failed attempts

The table witness's `--both` run observed native exit 0 but Terminal refused:
`native scalar differs from its admitted logical/storage codec`. The unchanged
host adapter rejects `addr` scalar codecs and offers import-backed calls only.
No Terminal agreement, runtime-driver completion, or new Linux execution is
claimed. The existing Linux native row is historical evidence for its stamped
older source, not qualification of the changed program.

Earlier table runs exposed the x86-only encoder and incomplete operand replay;
neither is relabeled a success. An initial wrong-table command used native
mode, which excludes fail fixtures, and matched none; the ordinary check route
above reaches the intended target realization. A newly authored write-only
table control refused at an earlier missing boundary-claim obligation. It was
removed rather than changing its oracle to count that unrelated refusal.

## Actual integration and required checking repair

Landing encountered a real conflict with newer main. The merge of published
`5b3373427d83e24f434db394f4174cb37709a2c0` produced
`7ebdc5f3adcb0cd79b6e1e562507bd5c6a8d3f01`. Its table/native-layout checks
passed, but the private-callback witness refused `bytes.unknown` in checking.
New member rebasing correctly rejected an invalid nominal field that builtin
collection length had incorrectly emitted. A smaller stored-slice witness
reproduced `alias.view.unknown` with that integrated producer.

The repair records authenticated metadata as a read of its evaluated receiver's
descriptor, not a nominal member or the pointee contents. The existing typed
descriptor walk retains crossed-receiver lineage and every alternative, follows
nested references to the slice descriptor, and reads no elements for fixed-array
geometry. Both ordinary reads and call-argument accesses join the producer's
exact metadata observation and post-selector constraint cut. Scalar completion
uses checked result types rather than source place projections. The invalid
nominal-field guard remains unchanged. A live exclusive holder still excludes
reading its stored descriptor; the new negative demonstrates that refusal.

The final production source is
`e2e21da6e1d99f2c66fb994e235d42e8e93f517f`, with verified opt1 runner SHA-256
`dd91c4c2f13373fca06accad77188494d84c26bceb83af6e0a8398a99f750205`.
[Integrated receipts and source](https://github.com/CathedralOS/Omega/releases/tag/evidence/pr1436-source-e2e21da6e1d9)
preserve the failed integration, partial repair, and final observations separately.
The final 29 observations over 28 distinct fixtures contain 12 native/Terminal
agreements, 14 intended refusals, two native successes and one positive check.
These include every existing length-row place and its nine reject twins, the
new nested/projected descriptor witness and suspension control, the original
descriptor-loan controls, private-callback controls, table calls and native
layout controls. Execution observations are macOS ARM64, not other-host results.

The existing shared-target test passed in 128.29 s. Its counters prove one Source
judgment, one Build execution and one Terminal publication; pointer identity
proves shared storage. Mixed-target refusal preserves the surviving output,
and both macOS ARM64 and Linux ARM64 native images match their respective
single-target images byte-for-byte. Only the matching macOS image executes.
These timings are observations across changed upstream sources, not a speedup
or a controlled performance comparison. Private-callback qualification took
103.327 s; this remains an expensive focused workflow.

The parent work remains open: capture Calling request/review and helper inputs
once, remove per-target Source preparation, complete the remaining provider
consumers, then establish Cathedral's fresh UEFI handoff baseline.
