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
| Local | [program](local/main.omg) | [reject](invalid_local/main.omg) |
| Field | [program](field/main.omg) | [reject](invalid_field/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](invalid_parameter/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](invalid_call_result/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](invalid_match_join/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](invalid_const_name/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](invalid_other_state/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](invalid_other_machine/main.omg) |
| Selected target body | [program](target_body/main.omg) | [reject](invalid_target_body/main.omg) |

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

[The Linux row run](https://github.com/CathedralOS/Omega/actions/runs/41973178090)
checks all nine tagged-case programs, runs each at native/Terminal exit 0 in the
corpus and as an ordinary sample, and rejects all nine local-backing twins.
It also rejects an out-of-family width, a missing family row and unproved tuple
forwarding. Independent review verified the raw reports, all 316 frozen inputs,
both optimized executables and both retained source bundles. The programs are
at `555bdbb56368426ed961fd76f613a32315ab44f1`; the authenticated compiler remains
`44f1471e7e5f9e96599b8d0f68dd8b11d053f687`.

The final contract-join code at `f37c9cfc83e25c784206b0b60fb5d829f5295499`
passes [fresh Linux validation](https://github.com/CathedralOS/Omega/actions/runs/41975158564):
all nine places and the focused program check and run at native/Terminal exit 0,
all twelve controls reject correctly, and the eighteen sample outcomes pass.
The focused program also passes ordinary package review and both engines.

[Linux ARM](https://github.com/CathedralOS/Omega/actions/runs/41973539242)
passes the same scope with compiler and programs at
`555bdbb56368426ed961fd76f613a32315ab44f1`. Independent review verified both hosts'
raw results and frozen inputs. It also found that the final join change keeps
the same lowering operations and emitted contracts for every admitted row;
only already-refusing contract disagreements can report a different first error.
The ARM observations keep their original source identity.

[Windows](https://github.com/CathedralOS/Omega/actions/runs/41973546386) and
[macOS](https://github.com/CathedralOS/Omega/actions/runs/41973543038) also pass
that complete scope at the same original `555bdbb56368426ed961fd76f613a32315ab44f1`
source. Each host's raw reports, executable profiles, frozen inputs and ordinary
CLI engine markers passed independent review. All four hosted targets therefore
have actual observations; only Linux executes the final contract-join refactor.
The other three retain their original stamps and the reviewed applicability
argument above.

The two unchanged Std-based controls
[time out with the published pre-repair compiler](https://github.com/CathedralOS/Omega/actions/runs/41974279825).
The later [checking repair](https://github.com/CathedralOS/Omega/pull/1900)
reaches both intended contract errors at their original limits; its fresh
Linux observations are described in the [cleanup row](../finite_family_exclusive_result/README.md#restored-contract-checks).
The older timeout reports remain failed, and their results retain their actual
compiler identities.
Goldens retain the old timing when the recorded behavior is unchanged; fresh
raw elapsed times remain in their reports.

Earlier complete plain-record results remain historical in the
[Linux report](https://github.com/CathedralOS/Omega/actions/runs/41829998724)
at `9ad31e5e871` and the other-host reports retained with the existing repair.
Older reports also cover projects that were retired; they do not describe this
folder's current eighteen projects. Their raw reports and source identities
remain in the original PR evidence.

This row does not close either parent task. More ways to store or forward returned values,
compile-time values that refer to stored bytes, other ownership modes and the
Omega-written compiler still need their own checks. No owner design decision is needed for this repair.
