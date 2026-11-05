# Borrowed views returned through selected cases

A runtime-selected function returns a view of its caller's bytes inside one of
two cases. The caller checks the selected case, then reads the original first
and last bytes. The programs keep the same input views as they pass through the
nine places below.

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md).
The explicit target selections also contribute evidence for
**BUILD-OWNED-TARGET-SELECTION**.

Fact: A finite-family method on a borrowed record returns a descriptor of the original backing, and reading its first byte yields 11.

`Packet` carries a shared byte view. `Returned` contains a `Packet` in either its
`Narrow` or `Wide` case. The selected method returns `Narrow` for width 16 and
`Wide` for width 32. Both cases must keep the input's view. The public trait names
the complete input and output types with the same declared lifetime. Each
implementation must prove that relation. The selector chooses code and contains
no user data.

The two caller arrays remain `[11,22,33]` and `[11,44,55]`. Every input is checked
at both widths. An independent width argument checks the returned case, and the
last-byte expectation distinguishes the two original backings. A fixed case or
a substituted input cannot pass these checks. A length guard proves indexing
safe; it does not establish the returned view's lifetime.

| Place | Program | Invocation-local backing control |
| --- | --- | --- |
| Local | [program](local/main.omg) | [reject](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [reject](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| Selected target body | [program](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

Each twin changes only the method's returned backing to a local array. It must
reject because that array does not outlive the call. Main/Build copies are pinned
under `tests/omega/{pass,fail}/facts/finite_family_returned_descriptor/`.
All positive programs bind the four hosted targets and expect exit 0.

These places transport the input `Packet` before the common selected call.
The field place also checks its neighboring values 17 and 23. The match place
runs both choices with independently supplied endpoint expectations. The const
place copies named arrays into runtime storage before borrowing them; it does
not hold a runtime borrowed result in a const binding. The target place uses
ordinary implementations selected through explicit Build rows.

## Compiler requirement

Each source call can have its own private slot symbol while requiring the same
runtime family row. Lowering must compare every checked call-shape field,
including guarantees about individual fields and the complete result-to-input
borrowing relation. It
must also lower each plan and compare the complete Terminal contract, because
scalar qualifications still depend on its own symbol. Missing or conflicting
contracts reject; an implementation's signature cannot replace the public one.

The public lifetime relates each active case's view to the original input.
Construction, selection, forwarding, field reads and cleanup preserve that
relation without borrowing private carrier storage. Calls and native images
must retain the case, address and length so Terminal and native execution read
the same original bytes.

These obligations follow [finite method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families),
[returned views](../../../../wiki/spec/language/lifetimes.md#returned-views),
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md)
and [provider selection](../../../../wiki/spec/build/provider_selection.md).

## Evidence and remaining work

The focused [tagged-result program](../../../../tests/omega/pass/traits/finite_family_case_view_result_exit/main.omg)
failed lowering with the published `3fdbcaba2c8fea5695bbb0336adb0ce473e0eaef`
compiler, reporting multiple checked public call shapes for one tuple.
[Run 41967472108](https://github.com/CathedralOS/Omega/actions/runs/41967472108)
retains that frozen program and the authenticated compiler; neither engine ran.

With repaired compiler `44f1471e7e5f9e96599b8d0f68dd8b11d053f687`,
[run 41968764734](https://github.com/CathedralOS/Omega/actions/runs/41968764734)
checks the focused program and original plain-record local place, then runs both
at native/Terminal exit 0. Ordinary package review and execution also pass for
the focused program. The existing local-backing twin rejects correctly. The
whole report remains failed: two unchanged Std-based family controls time out
without a diagnostic. They provide no current rejection evidence.

The tagged-case versions of all nine places and twins are now prepared.
Their checking and execution on all four hosts remain pending. No result from
the focused program is promoted to these changed programs or to a newer main.

Earlier complete plain-record results remain historical in the
[Linux report](https://github.com/CathedralOS/Omega/actions/runs/41829998724)
at `9ad31e5e871` and the other-host reports retained with the existing repair.
Older reports also cover projects that were retired; they do not describe this
folder's current eighteen projects. Their raw reports and source identities
remain in the original PR evidence.

This row does not close either parent task. More ways to store or forward returned values,
compile-time values that refer to stored bytes, other ownership modes and the
Omega-written compiler still need their own checks. No owner design decision is needed for this repair.
