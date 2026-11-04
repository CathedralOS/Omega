# Keep array sizes and endpoint bytes through a selected call

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

An actual record conflict required merging main `9e959fe1460e` into measured
source `7ea5b8217acc27c062f0a479cd915ea98f62bfeb`. All 343 authored inputs and
58 matching Main/Build pairs remain unchanged. Main changed domain and call
checks used by the wrapping counter, UTF-8 snapshot and qualified boundary
arguments, so those consumers were checked again.

| Host | New corpus checking | New native/Terminal results | New ordinary fact projects |
| --- | --- | --- | --- |
| Linux x86-64 | 28 checked, 45 intended refusals | 28 agreements, 2 publication refusals | 14 exit-zero agreements, 14 source refusals, 1 publication refusal |
| macOS ARM64 | 6 checked, 2 intended refusals | 6 agreements, 1 publication refusal | 14 exit-zero agreements, 14 source refusals, 1 publication refusal |

The Linux [seed run](https://github.com/CathedralOS/Omega/actions/runs/41754803449)
passed all 29 ordinary projects. Its [complete corpus replay](https://github.com/CathedralOS/Omega/actions/runs/41757296977)
used the identical verified CLI and runner, without rebuilding Rust. The
[Mac run](https://github.com/CathedralOS/Omega/actions/runs/41754806687) completed
the affected qualification, borrow, boundary and policy controls, then reached
the workflow limit during ordinary projects. It remains canceled. A
[separate replay](https://github.com/CathedralOS/Omega/actions/runs/41761785405)
passed all 29 ordinary projects using its identical authenticated CLI, without
rebuilding Rust. Every positive in this fact exits 0 in both engines.
The Linux agreements include four additional programs' authored exit 70;
the Mac agreements include two. Publication refusals have no engine result.

The current checking records and Linux engine records use the actual `7ea5b8217acc`
execution identity. Mac engine records combine seven new observations with 23
applicable retained observations at `990cae4277cb`. The two supplied Linux ARM64
observations also remain at `990cae4277cb`. Independent review accepted that
reuse after inspecting the changed consumers: between those measured sources,
no library, backend, interpreter, provider-selection or Terminal format code changed; all required fact projects
ran again on both full hosts, and affected Mac controls ran again. No retained
row is described as a new-source execution.

The [supplied Linux ARM64 pair](https://github.com/CathedralOS/Omega/actions/runs/41734060102)
has actual exit-zero agreements for both corpus programs and both ordinary
projects. Its separate checking leg used macOS ARM64 and refused missing supply
there. This qualifies only that pair. Windows native execution and the rest
of the row on Linux ARM64 remain unmeasured here.

The private-call cases include the unchanged bounds loop and guard-floor
program, a private division helper and a guarded enum helper. A checked false
caller guard may exclude its exact private call without inventing helper inputs.
Every possible call still needs an actual entry or its own checked exclusion.
The [live size-zero control](../../../../tests/omega/fail/generics/const_extent_guard_live_zero_helper/main.omg)
changes only the caller guard and still rejects its reads. Mixed-arrival,
unknown-input, unguarded-cycle and partial-operand controls retain their causes.

Ordinary publication controls first pass the portable source check, then refuse
target compilation for the exact missing provider. The corpus checking route
also evaluates target publication. All producer profiles use optimization
level 1. Every unrelated recorded block from incoming main is preserved, and
the final README and record updates change no compiler, program or oracle bytes.
The eight shared publication-oracle tool, test and documentation files retain
their 219 focused Python results and two existing skips.

Independent source and raw reviews and exact source archives are retained with
[PR #1721](https://github.com/CathedralOS/Omega/pull/1721). Exact-candidate
review decisions keep their own submitted source identities.

<details>
<summary>Original baseline and earlier measured sources</summary>

The earlier `990cae4277cbd9182e82d58f4fb508c9c2394af9` source passed the full
73 checking and 30 native/Terminal fixtures on both full hosts. Each host had
24 exit-zero agreements, four authored exit-70 agreements and two publication
refusals. All 29 ordinary projects also met their oracles. Those observations
remain at their actual source identities.

The [Linux corpus job](https://github.com/CathedralOS/Omega/actions/runs/41731569787)
completed both corpus legs, then reached the workflow limit during ordinary
projects. It remains canceled. A [separate ordinary replay](https://github.com/CathedralOS/Omega/actions/runs/41740343841)
passed all 29 projects using the exact retained CLI. The original
[Mac job](https://github.com/CathedralOS/Omega/actions/runs/41731574468) was canceled
without usable artifacts and supplies no evidence. A [Mac seed](https://github.com/CathedralOS/Omega/actions/runs/41741397550)
passed all 29 projects; its [complete corpus replay](https://github.com/CathedralOS/Omega/actions/runs/41746623572)
passed with the identical CLI and runner. Case limits, oracles and worker counts
were unchanged.

The earlier a018 [Linux](https://github.com/CathedralOS/Omega/actions/runs/41697815800) and
[macOS](https://github.com/CathedralOS/Omega/actions/runs/41697821789) runs passed the 57-fixture core and all 29 ordinary
projects after the preceding main conflict. Its
[Linux ARM64 pair](https://github.com/CathedralOS/Omega/actions/runs/41697827253) passed only that pair's supplied-target
scope. Those observations keep their original source identities.

The later 3bb [Linux](https://github.com/CathedralOS/Omega/actions/runs/41718008302) and
[macOS](https://github.com/CathedralOS/Omega/actions/runs/41718012273) jobs remain failed. They contained valid supplied-policy
and storage observations, but the new provider-choice control still had
unsupported arm syntax. An additional returned-record probe declined in Terminal
on Linux and timed out on macOS. Its identical Linux decline was reproduced with
[a018](https://github.com/CathedralOS/Omega/actions/runs/41722538330) and a [retained compiler](https://github.com/CathedralOS/Omega/actions/runs/41724653440) matching
published main `108f8a95dcd8`. That probe supplies no current passing coverage;
no current outcome is inferred for it. Its source, oracle and historical records
remain retained. The existing owned-snapshot program supplies the additional
byte-field replacement, saved-copy and disjoint-sibling evidence instead.

At a2b, the corrected provider-choice control and all 29 projects passed on
[Linux](https://github.com/CathedralOS/Omega/actions/runs/41722369908) and [macOS](https://github.com/CathedralOS/Omega/actions/runs/41725743952).
The separate snapshot check passed with native and Terminal exit 70 on
[Linux](https://github.com/CathedralOS/Omega/actions/runs/41723253579) and [macOS](https://github.com/CathedralOS/Omega/actions/runs/41723406704).
These earlier results remain historical; their actual source stamps remain in
the retained reports. Earlier c831 diagnostic and malformed-control failures,
canceled runs and review corrections remain retained as well.

The original source is frozen at `d24ceb05541522559a6a41c9e8075390c18daea9`, whose
tree equals published `e283856d96276d4c262d9b6c3f687584942da6c3`. Its scope is 28
projects and 28 pins, with 196 authored input hashes and 56 matching Main/Build
pairs. The [Linux](https://github.com/CathedralOS/Omega/actions/runs/41576490895)
and [macOS](https://github.com/CathedralOS/Omega/actions/runs/41576493746) reports
each record 14 checked positives, 14 intended corpus refusals and 14
native/Terminal exit-zero agreements. Both jobs failed the ordinary sample leg:
14 programs passed, 13 source controls matched, and `uncalled_target/invalid`
passed a source-only check instead of matching its target-qualification refusal.
Its corpus diagnostic came from native execution inputs. It did not execute.

The strengthened local program then exposed a separate defect with the same
verified baseline compiler: both writes to byte 1 refused despite the member's
width-16-or-32 contract. The new boundary bridge also failed to account for its
reference-free generic byte array. Those sources and diagnostics were frozen
before compiler edits.

At `b35c9948bf17346df96893fee2b79606a7206f65`,
[Linux](https://github.com/CathedralOS/Omega/actions/runs/41672705417) and
[macOS](https://github.com/CathedralOS/Omega/actions/runs/41672714587) ran all 14
fact positives with exit 0 in both engines. Missing supply still wrongly checked
and built. The runs also found a bounds-loop regression, a premature refusal in
a reference control, and three stale negative expectations, so neither reached
ordinary sample qualification. Those failures remain historical observations.

The corrections keep a specialized array's checked literal length instead of
replacing it with a template parameter. They also require authored target supply;
an implementation merely being in scope cannot fill an omitted Build row.
The full f46 runs remained red only on five reads in the guard-floor program,
which also failed with the fixed-base compiler. The current private-call repair
makes that unchanged program check and execute successfully.

Two negative expectations needed separate attribution. A negative index already
refuses when -1 cannot form an unsigned index, before bounds checking. In the
combined symbolic-call control, one function fails during loan formation and
prevents a later pass from reporting the other function. Each unchanged function
independently rejects with the fixed-base compiler. The combined program stays;
[symbolic-first](../../../../tests/omega/fail/borrows/symbolic_call_before_concrete_output_obligation/main.omg)
and [concrete-first](../../../../tests/omega/fail/borrows/concrete_call_before_symbolic_output_obligation/main.omg)
controls preserve both source orders. No duplicate errors are invented to match
a former count.

Earlier documentation reports Linux executions after fixes to family identity,
argument substitution and row-specific call shapes, plus a macOS x86-64 target
migration at `11bdd2fe41f`. Those observations belong to their original source
and do not qualify these changed programs.

</details>

Raw reports, frozen input hashes, published source archives and current
qualification records remain under `build/finite-inline-selected-20261103/`.
This row does not close either parent task. Other hosted runtimes,
product-compiler parity, automatic cleanup and other finite-family requirements
remain open. No owner design decision is needed for this migration.
