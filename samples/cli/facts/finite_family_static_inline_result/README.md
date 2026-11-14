# Keep array sizes and endpoint bytes through a selected call

These programs pass arrays of 16 and 32 bytes through a function selected at
runtime. Each result must keep its exact array size and the input's first and
last bytes. Each nested function that supplies an input must run once.

Owning tasks: **MODULE-NAMESPACE-RESOLUTION** (including
**FINITE-GENERIC-DISPATCH**) and **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).

A literal finite-family call preserves its selected inline result shape and the first and last bytes of its input while evaluating each nested input producer once.

The declared family contains both widths, regardless of which call the program
makes. The requirement owns that complete set; a caller cannot remove an unused
row or use an implementation's private type as the public result type. This
follows [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary)
and [dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families).

The 16-byte input has endpoints 11 and 23; the 32-byte input has endpoints 17 and
29. `Selected` writes 31 at interior byte 1, while `Other` writes 47 there. The
caller checks those markers on the actual returned arrays, alongside their
endpoints and the input-producer count of two. Choosing the same implementation
for both calls cannot satisfy these checks. The public types and promised
endpoints are unchanged.

The match place also puts 3 or 7 in input byte 2. Its caller supplies the expected
value independently and exercises both choices. The returned byte must match
that expectation. An inverted match or a fixed arm cannot pass merely because
both original arms had the same endpoints.

## Places and controls

| Place | Valid program | Wrong selected width |
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

Each selected call runs inside a branch and returns through an explicit finite
sum. Its cases retain the exact 16-byte and 32-byte payload types. The caller
stores and forwards both results, then checks the independently expected case,
array endpoints and implementation marker. A snapshot after the first call
must be one; the final producer count must be two.

Every twin changes only the narrow branch's selected width from 16 to 32.
The input still contains 16 bytes, so it cannot satisfy that call's 32-byte
parameter. Both versions retain the same member bodies and runtime checks.
The existing rejection pins remain unchanged. A [focused constructor
control](../../../../tests/omega/fail/generics/runtime_index_carrier_constructor_wrong_payload/main.omg)
keeps both selected calls valid and changes only the wide output case to
`Narrow`; its 32-byte result cannot inhabit a case declared to contain 16 bytes.
The [local positive](local/main.omg) is that control's valid counterpart.
Each valid program binds the four required hosted targets and expects exit 0;
the existing target place also retains its optional macOS x86-64 binding.

## Requirements proved here

Public contracts establish the allowed widths and each call's exact input and
result type. The checker must substitute the actual width into the array field
and prove interior index 1 safe for both alternatives. A caller's width binder
and the data declaration's binder remain distinct declarations even when their
names match.

The arrays carry values through the nine input places into the common selected
calls. These are nine input transport places and one common finite-sum return
path, rather than nine different forms of sum storage. Each call must preserve
its array size, endpoints and implementation marker through the returned case,
Terminal Psi, calling plans and native execution. The producer
counter checks that nested input evaluation occurs once. The const place stores
copyable input arrays under const names; it does not transport an exclusive
result through a const binding.

The target-body place selects an x86-64 or ARM64 pair producer through explicit
Build rows. Each provider checks tag 17 or 29 through `SelectedBodyCheck`, so
swapping providers traps. These declarations follow
[provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection),
[multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation)
and [service reach](../../../../wiki/spec/language/effects.md).

The runtime checks observe preserved values and effects. They do not supply a
missing type, bounds proof or provider. This row covers copyable byte arrays;
exclusive ownership, automatic cleanup, runtime-sized results and complete
family supply have separate requirements.

## Validation and remaining scope

All nine current programs and their nine reject twins are mirrored under
`tests/omega/{pass,fail}/facts/finite_family_static_inline_result/`.
The current programs return the selected array through the explicit case value
described above. Their Build declarations and expected exit remain unchanged.

| Host | Compiler source | Checking | Corpus execution | Sample execution |
| --- | --- | --- | --- | --- |
| Linux x86-64 | `6d20220933d71e8aad836e69687751c92ebcd2bc` | 9 valid, 9 intended rejects | 9 native/Terminal exit-zero agreements | 9 agreements, 9 intended rejects |
| Linux ARM64 | `64376af6ae978b4ddb2d2d50874724226ccb0646` | 9 valid, 9 intended rejects | 9 native/Terminal exit-zero agreements | 9 agreements, 9 intended rejects |
| macOS ARM64 | `64376af6ae978b4ddb2d2d50874724226ccb0646` | 9 valid, 9 intended rejects | 9 native/Terminal exit-zero agreements | 9 agreements, 9 intended rejects |
| Windows x86-64 | `64376af6ae978b4ddb2d2d50874724226ccb0646` | 9 valid, 9 intended rejects | 9 native/Terminal exit-zero agreements | 9 agreements, 9 intended rejects |

The [Linux run](https://github.com/CathedralOS/Omega/actions/runs/42106651463)
also checked the focused malformed-output and stale-index controls. In total,
30 valid programs checked, 29 controls rejected for their intended reasons,
and 16 programs returned the authored zero in both engines. The Source report
retains its timing movements against the stored baseline. The whole run failed
because its ordinary CLI setup passed a relative workspace path before
compilation. A [focused replay](https://github.com/CathedralOS/Omega/actions/runs/42113960599)
with that path corrected passed package admission and returned zero in both
engines, using the same authenticated optimized compiler. It did not repeat or
relabel the completed checks.

The [Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/42115153222),
[macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/42115149060) and
[Windows x86-64](https://github.com/CathedralOS/Omega/actions/runs/42115157698)
runs also passed checking, both execution engines, all sample projects and
ordinary package admission. Their full reports and actual products are retained.
Each checks the same nine programs and nine twins, executes the nine positives
in both engines, and exercises ordinary package admission. Their compiler and
program inputs match the completed Linux work. Results keep the actual producer
and host; Source checking uses the existing macOS ARM64 check target, while
execution uses each host's actual target.

Independent source review confirmed that the final lowering cleanup in
[PR #1919](https://github.com/CathedralOS/Omega/pull/1919) leaves the proof references
in these programs unchanged. Results stay stamped with the compiler revisions
actually used.

Earlier [Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41961822390)
and [Windows](https://github.com/CathedralOS/Omega/actions/runs/41961828697)
results used program source `51ea1c54b2cfd7695dd666ae61cb55c27f8f61c8`, before
the explicit case return. Earlier [Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41783441526)
and [macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/41784898410)
results and their stored corpus records also remain historical. These results
are retained at their original sources; they do not validate the changed return
path. The explicit provider choices came from
[PR #1789](https://github.com/CathedralOS/Omega/pull/1789).

The malformed output case previously reached a late lowering refusal despite
having the wrong payload type. It now fails source checking for that mismatch.
The saved-index pair separately checks that a returned value retains the index
captured before a later write. These controls keep their original programs and
rejection pins.

Both parent tasks remain open. This row covers copyable arrays and this case
return. Exclusive ownership and cleanup, escaping runtime-sized results,
complete family supply, remaining source migrations and product-compiler support
remain separate requirements.
