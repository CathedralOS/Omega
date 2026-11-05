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
| Local | [program](local/main.omg) | [reject](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [reject](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| Selected target body | [program](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

Each selected call runs inside a branch and returns through an explicit finite
sum. Its cases retain the exact 16-byte and 32-byte payload types. The caller
stores and forwards both results, then checks the independently expected case,
array endpoints and implementation marker. A snapshot after the first call
must be one; the final producer count must be two.

Every twin changes only the wide result's output case from `Wide` to `Narrow`.
Its input and selected 32-byte call stay valid. The 32-byte result cannot inhabit
the case declared to contain 16 bytes. Both versions retain the same member
bodies and runtime checks. Each valid
program binds the four required hosted targets and expects exit 0; the existing
target place also retains its optional macOS x86-64 binding.

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

The current folder contains nine valid programs and nine reject twins, mirrored
under `tests/omega/{pass,fail}/facts/finite_family_static_inline_result/`.
The current programs add the explicit finite-sum return described above. The
Build declarations and valid-program exit remain unchanged. Validation of this
change is in progress; the table below records the earlier programs only.

| Host | Compiler source | Fresh checking | Fresh corpus execution | Fresh sample execution |
| --- | --- | --- | --- | --- |
| Linux ARM64 | `ed656648a42a42ed65cfb991f1ac8941a7dbd558` | 9 pass, 9 intended rejects | 9 native/Terminal exit-zero agreements | 9 agreements, 9 intended rejects |
| Windows x86-64 | `46cad3817d2fbb2d03aaa45b5fae6f68b89ad0ad` | 9 pass, 9 intended rejects | 9 native/Terminal exit-zero agreements | 9 agreements, 9 intended rejects |

The [Linux ARM64 run](https://github.com/CathedralOS/Omega/actions/runs/41961822390)
and [Windows run](https://github.com/CathedralOS/Omega/actions/runs/41961828697)
use program source `51ea1c54b2cfd7695dd666ae61cb55c27f8f61c8`.
Both reuse authenticated optimization-level-1 executables from their recorded
compiler sources. All compiler inputs and all 306 frozen program, fixture and
library inputs match those sources. No Rust compiler build was needed. The
workflow-only dispatch commits are recorded separately from the programs and
compilers. Source checking uses the existing macOS ARM64 check target; execution
uses each host's actual target.

Earlier full-row Linux x86-64 and macOS ARM64 observations remain at their
original sources in the [Linux](https://github.com/CathedralOS/Omega/actions/runs/41783441526)
and [Mac](https://github.com/CathedralOS/Omega/actions/runs/41784898410) reports
and existing corpus records. They are historical results, not fresh execution
with either compiler above. The later explicit provider choices came from
[PR #1789](https://github.com/CathedralOS/Omega/pull/1789).

Earlier reports also include programs that have since been retired. Those
reports remain historical evidence in [PR #1721](https://github.com/CathedralOS/Omega/pull/1721);
they do not describe the contents or acceptance of this folder today.

Those earlier results filled the previously unmeasured Windows row and
remaining Linux ARM64 places. They establish coverage of the earlier programs;
they do not validate the new finite-sum return path. Both parent tasks remain open for their wider acceptance,
including ownership and cleanup, escaping runtime-sized results, remaining
source migrations and product-compiler support. No owner design decision is
needed for this coverage update.

The new local program already checked and ran with native/Terminal exit-zero
agreement before the checking repair. Its malformed output case instead reached
a late unlowerable-body refusal. A focused repaired-compiler check now reports
that the payload cannot retain a different captured index. This is a missing
source check, not evidence that the old compiler executed the invalid program.
The complete row and current cast traversal still require execution validation.
