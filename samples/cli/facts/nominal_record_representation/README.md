# Nominal record representation

Fact: a declared record named string retains its nominal record representation and value >= 7 through transport, discharging value - 7 while borrowed field access preserves adjacent values.

Owning task: `PIPELINE-ARCHITECTURE-ENFORCEMENT`.
Claim ticket: `e7ea8d46d21149ebabf29142d37cba24` (`Cloud six / pipeline`).

The name `string` is an ordinary nominal declaration. Text views use the actual
slice representation. The witness keeps three `u64` fields, passes a borrowed
record beside a second argument, returns two independently observed values,
and checks both neighboring fields. On the four hosted 64-bit targets its
record occupies 24 bytes and a reference occupies 8 bytes. Every place consumes
the same `value >= 7` declaration fact with exact subtraction `value - 7`.
The match join exercises both arms with distinct results, 2 and 4. The target
place uses `SelectedRecordOperation`, an ordinary named implementation selected
by `Build::select_provider` for each hosted target.

Each twin changes only the constructed nominal type to `Other`, whose fields,
values and default-domain predicate are identical. The required `string`
identity stays unchanged at the receiving position. It must refuse that exact
nominal mismatch, without treating either record as text or admitting a
structurally identical substitute.

| Place | Witness | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

Every positive project declares an exit-0 oracle and ordinary Build bindings
for `linux_x86_64`, `linux_arm64`, `macos_arm64` and `windows_x86_64`.
Identical source and Build files are pinned under
`tests/omega/{pass,fail}/facts/nominal_record_representation_through_<place>_*`.

Run a witness with `omega run --both <place>/main.omg`; run its twin with
`omega --check <place>/invalid/main.omg`. Foreign bindings are authored coverage,
not a claim of execution on an unavailable host. The existing
`slice_view_layout_geometry` control separately retains two-word slice views.

## Measured local acceptance

On `linux_x86_64`, all nine positive projects execute with `native exit 0` and
`interp exit: 0`. All nine twins reject their exact `string`/`Other` nominal
mismatch at the owning landing position. The optimized producer has SHA-256
`11c798e0319051245654fadebcacbfd90d16ec0386eed4ed14a503f0705bc5d7`.
The retained local evidence includes each Terminal product, native image,
identity and fuel receipt, plus source/oracle and artifact hashes. Other hosted
platforms have not been executed in this Linux cloud session.

Changing only the local witness's demanded result from 2 to 3 produces native
signal 4 and Terminal `Trap` with host-presentation agreement. The frozen
pre-repair source also succeeds unchanged in both engines. An existing slice
layout witness still observes its two-word descriptor in both engines.
