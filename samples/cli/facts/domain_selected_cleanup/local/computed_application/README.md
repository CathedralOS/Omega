# Computed cleanup applications

Owner: **STATIC-MACHINE-MIGRATION** in [TASKS.md](../../../../../../TASKS.md).
This is another production of the [existing cleanup fact](../../README.md),
with the same code-7 obligation and once-only write to the caller's backing.
The static cleanup argument is `Code + 0`, retained from the domain declaration
until the exact `Held<7>` application closes it. No new fact folder or wider
numeric test matrix is introduced.

The fixed base `e9f492c5ccc6f84731ce0e65eb9a9d4f56fdb067`, built at
optimization level 1, rejects the frozen local witness with
`cleanup application release_keeper must supply a complete consistent static tuple`.
Resolution omitted value expressions in declaration-owned static operands, so
`Code` initially had no symbol. The ordinary const tuple collector also
discarded the open expression binding. Resolution now visits those operands in
their owning header scope; tuple collection retains the exact expression and
uses the shared closure query to defer executable selection. Immutable checking
forms declaration-owned recipes with the existing declaration judgment, using
only the supplier's telescope and facts. Machine recipes retain their runtime
prefix judgment. Closed values still owe ordinary const admission and hook
prerequisites, and binder lookup no longer scans unrelated machine names.

The field place also exposes contextual construction handback. Its parent field
retains a provisional membership only for fresh owned constructions with complete
inline predicate dependencies, including ordinary scalar comparisons; the earlier
child capture independently owes the
construction proof. Projected owned children retain the parent completion as
their observation and their earlier capture as producer. Live views keep their
own referents. Package review uses the declaration's exact index roster, so
fixed-carrier domain arguments are not mistaken for types when publishing the
selected implementation's signature.

| Place | Positive | Code-6 twin |
| --- | --- | --- |
| local | [source](main.omg) | [source](invalid/main.omg) |
| field | [source](../../field/computed_application/main.omg) | [source](../../field/computed_application/invalid/main.omg) |
| parameter | [source](../../parameter/computed_application/main.omg) | [source](../../parameter/computed_application/invalid/main.omg) |
| call result | [source](../../call_result/computed_application/main.omg) | [source](../../call_result/computed_application/invalid/main.omg) |
| match join | [source](../../match_join/computed_application/main.omg) | [source](../../match_join/computed_application/invalid/main.omg) |
| const name | [source](../../const_name/computed_application/main.omg) | [source](../../const_name/computed_application/invalid/main.omg) |
| nested machine | [source](../../other_state/computed_application/main.omg) | [source](../../other_state/computed_application/invalid/main.omg) |
| other machine | [source](../../other_machine/computed_application/main.omg) | [source](../../other_machine/computed_application/invalid/main.omg) |
| selected implementation | [source](../../target_body/computed_application/main.omg) | [source](../../target_body/computed_application/invalid/main.omg) |

Every entry runs two cleanup operations on distinct caller-owned Counters.
Initial invocation counts 0 and 9 must become 1 and 10, and both values must
become 7. Neighbor canaries 317 and 929 must survive. The match place runs
both alternatives with independent expected tags 11 and 22. The selected body
retains the existing x86/AArch64 tag checks and explicit Build provider rows.
Each twin changes only the owned code from 7 to 6, including the named constant
in that place; it must reject the missing `Held` domain obligation.

The governing contracts are [exact cleanup selection and intact transport](../../../../../../wiki/spec/language/domains.md#domain-selected-automatic-cleanup),
[symbolic contextual cleanup](../../../../../../wiki/spec/language/ownership.md#consumers-and-cleanup),
and [static generic applications](../../../../../../wiki/spec/language/generics.md#applications-and-specialization).

- Establishment: construction independently proves code 7 before establishing
  `Held<7>`; the selected hook still requires code 7.
- Identity and lifetime: the owner retains its exact exclusive Counter backing;
  cleanup must release that window before the caller observes the result.
- Composition: the same application and operation travel through all nine places;
  two match arms and ordinary call/return/field transports retain the obligation.
- Soundness: retaining a recipe grants neither a concrete specialization nor
  permission to erase cleanup, misclassify a Type argument, or overflow a carrier.
- Psi 05–07: the exact selected hook and owned subject must survive ordinary
  cleanup production and canonical Terminal execution.
- Omega 00–02: all four hosted ProgramEntry bindings are authored; the selected
  implementation uses ordinary target-keyed Build providers and calling plans.
- Omega 03–06: observed writes, invocation deltas and neighboring canaries falsify
  lost hooks, duplicated calls and damaged backing; no new stack-bound claim.
- Omega 07–09: acceptance requires native/Terminal exit-0 agreement on the
  executing host. Authored bindings alone do not qualify foreign hosts.

A same-spelled global `Code` with value 41 and a differently named hook binder
challenge exact lexical identity. Additional controls reject conflicting selected
applications. Existing carrier-overflow controls remain part of focused regression
validation.

Pinned positives and twins are under `tests/omega/{pass,fail}/facts/` with prefix
`domain_selected_cleanup_computed_`. On Linux x86-64, all ten positive projects
(the nine places and lexical-identity control) exit 0 in native and Terminal
execution, and all ten invalid controls reject for their authored reasons.
The public outer command is `omega run --both <place>/main.omg`, after ordinary
per-project package review. A scratch counterfactual that expects invocation
count 2 instead of 1 produces native SIGILL and a Terminal `Trap`, establishing
that the count guard can falsify the runtime result.
The broader parent acceptance remains open: open caller-indexed application
closure, multiple generic-carrier specializations, product parity, source
retirement and other-host execution are not qualified by this closed-index row.
