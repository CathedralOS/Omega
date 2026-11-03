# Keep array sizes and bytes through a selected call

These programs pass arrays of 16 and 32 bytes through a function selected at
runtime. Each result must keep its exact array size and the input's first and
last bytes. Each nested function that supplies an input must run once.

Owning tasks: **FINITE-GENERIC-DISPATCH** and
**BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).

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

| Place | Valid program | Wrong tuple |
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

Every twin changes only the first selected tuple from 16 to 32 while retaining
its 16-byte input. The selected tuple exists, but the input cannot meet its exact
type. Both versions retain the same member bodies and runtime checks. Each valid
program binds the four required hosted targets and expects exit 0; the existing
target place also retains its optional macOS x86-64 binding.

The other original projects remain in the same qualification scope:

- [authored_sum](authored_sum/main.omg) keeps one concrete result type when the
  selected width varies at runtime.
- [nested_static](nested_static/main.omg) distinguishes nested const binders
  with the same spelling.
- [argument_effect](argument_effect/main.omg) and its [twin](argument_effect/invalid/main.omg)
  check completed input evaluation and the exact selected result index.
- [stale_input](stale_input/main.omg) and its [twin](stale_input/invalid/main.omg)
  distinguish a saved input from the value after an earlier operand changes it.
- [runtime_shape](runtime_shape/invalid/main.omg) and
  [nested_runtime_shape](nested_runtime_shape/invalid/main.omg) reject inline
  arrays whose runtime extent has no valid storage representation.
- [uncalled_target](uncalled_target/main.omg) and its
  [missing-supply control](uncalled_target/invalid/main.omg) require target
  support for the entire family, including the row that main does not call.

The added [bad interior index control](local/controls/out_of_bounds/invalid/main.omg)
uses index 16 under the unchanged width choices. It fits width 32 but is one
past the end of width 16. A proof that checks only the larger alternative must
not admit it. The original 28 projects are retained; this control brings the
sample scope to 29 projects.

## Build selection and missing supply

The `uncalled_target` program calls only row 16. Row 32 calls
`SelectedInlineTarget::target_only`, so creating the complete function table
still requires that target's supply. Four identical prefixed helpers become one
ordinary `PortableInlineTarget` implementation, selected by four explicit Build
rows. It returns its input unchanged.

The control supplies that implementation only for Linux ARM64 while retaining
all four entry bindings. Its source is valid. Linux ARM64 may execute with exit
0; Linux x86-64, macOS ARM64 and Windows x86-64 must refuse publication because
no provider is selected there. Target-specific publication and exit oracles
state those separate expectations. Publication refusal is not engine execution.
The corresponding pin is the build-only pass fixture
`tests/omega/pass/facts/finite_family_static_inline_result/uncalled_target_missing_supply`.

The existing `target_body` place still selects the x86-64 or ARM64 pair producer.
Each actual provider call checks tag 17 or 29 through `SelectedBodyCheck`, so a
swapped provider traps. That requirement uses no services; the pair requirement
and implementations now preserve empty service reach and name the synchronous
call separately with `invokes SelectedBodyCheck`.

These declarations follow [provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection),
[multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation)
and [service reach](../../../../wiki/spec/language/effects.md).

## Compiler requirements

The checker must interpret an array field's extent through the actual arguments
of its exact container. The data declaration's binder and a caller's binder are
different declarations, even when both are named `Width`. The shared bounds
judgment must prove index 1 safe for every alternative of `Width == 16 || Width == 32`.
The contract remains intact; no width-specific source bodies or caller guards
supply that proof.

An array of bytes also contains no borrowed references regardless of its count.
That proof does not establish an array's runtime size or storage. Reference-slot
checking can omit element enumeration only after complete borrow-free contents
and an empty retained-lifetime inventory are established. Pointer elements,
unknown element types, opaque data and erased reference fields retain their
independent restrictions.

Public contracts establish the family and exact input/output types. Ordinary
argument evaluation establishes the values and producer effects. Calls, stored
values, joins and nested machines must retain their exact tuple and actual data
through Terminal Psi, provider selection, calling plans and native emission.
The runtime byte/count checks observe that behavior; they do not establish a
missing static type or bounds guarantee.

## Validation and remaining scope

The original source is frozen at `d24ceb05541522559a6a41c9e8075390c18daea9`, whose
tree equals published `e283856d96276d4c262d9b6c3f687584942da6c3`. The full original
scope is 28 projects and 28 pins, with 196 authored input hashes and 56 matching
Main/Build pairs. Its [Linux](https://github.com/CathedralOS/Omega/actions/runs/41576490895)
and [macOS](https://github.com/CathedralOS/Omega/actions/runs/41576493746) reports
both record 14 checked positives, 14 intended corpus refusals and 14 native/Terminal
exit-zero agreements. The optimized producer profiles and downloaded report
hashes are retained.

Both jobs nevertheless failed the ordinary sample leg: 14 programs passed,
13 source controls matched, and `uncalled_target/invalid` passed its source-only
check instead of matching the target-qualification refusal. Its corpus diagnostic
came from native execution inputs. This is retained as failed baseline evidence,
not a green full gate or an observation that the control executed.

The strengthened local program then exposed a separate checking defect with
the same verified baseline compiler: both writes to byte 1 refused despite the
member's width-16-or-32 contract. The new boundary bridge also exposed an
incomplete reference-slot inventory for its reference-free generic byte array.
Those source snapshots and diagnostics are retained before compiler edits.
The first changed-source [Linux run](https://github.com/CathedralOS/Omega/actions/runs/41672705417)
at `b35c9948bf17346df96893fee2b79606a7206f65` observed all 14 fact positives
exiting 0 in both engines. The nine wrong-tuple twins, other original controls,
and added index-16 control rejected. The missing-supply control still checked
and built when it should refuse; it supplied no execution evidence. The run
also found a regression in an existing bounds loop and a premature refusal in
a reference control, so it stopped before ordinary sample qualification.
This is partial evidence, not a passing full scope.

The corrections keep a specialized array's checked literal length instead of
replacing it with its former template parameter. An index that cannot execute
may be accepted only when a complete observation of earlier control flow proves
that no path reaches it. Unknown or incomplete observations still refuse.
The five existing zero-length failures in the guard-floor program also occurred
with the fixed-base compiler; they remain part of the required validation.

The same run confirms that an implementation merely being in scope cannot fill
an omitted target row. The shared provider-selection repair requires authored
supply while retaining explicit overrides, declared defaults and native Build
bindings. That repair is now included; its effect on this control still needs
fresh measurement.

Two external rejection expectations needed investigation. A negative index
already refuses when -1 cannot form an unsigned index, before the later bounds
check. In the combined symbolic-call control, one function fails during loan
formation and prevents a later checking pass from reporting the other function.
Each unchanged function independently rejects with the fixed-base compiler.
The original combined program remains; separate
[symbolic-first](../../../../tests/omega/fail/borrows/symbolic_call_before_concrete_output_obligation/main.omg)
and [concrete-first](../../../../tests/omega/fail/borrows/concrete_call_before_symbolic_output_obligation/main.omg)
controls pin both source orders. No duplicate errors are manufactured to match
a former count. The current repairs and all ordinary projects still require
fresh qualification.

Earlier documentation reports Linux executions after fixes to family identity,
argument substitution and row-specific call shapes, plus a macOS x86-64 target
migration at `11bdd2fe41f`. Those observations belong to their original source.
They remain historical and do not qualify these changed programs.

Raw reports, frozen input hashes, source archives and current qualification
records remain under `build/finite-inline-selected-20261103/`. The original
baseline source is retained separately. This row does not close either parent
task. Other hosted runtimes, product-compiler parity, automatic cleanup and
other finite-family requirements remain open. No owner design decision is
needed for this migration.
