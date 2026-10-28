# Materialized C scalar-record results

Parent: [NORMALIZED-ABI-LOWERING](../../../../TASKS.md).

**A copyable C quotient/remainder record preserves both signed fields and
independent earlier-call contents through transport and copying into the
consuming operation.** Every place uses this same fact and libc `lldiv`.
The consumer passes the first whole record twice, calls C again with different
inputs, then reads all original, copied and later fields. The comparisons are
runtime oracles; they introduce no static quotient/remainder refinement.
Each reject twin removes only `[copy]` and must reject the second whole-value
use with `holds no value`. Thus the static obligation is copy custody, not a
post-hoc equality guard or an invented opaque-provider guarantee.

The nine places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [other machine](other_machine/main.omg),
and [target body](target_body/main.omg). Every place has `invalid/main.omg`;
matching copies are pinned under `tests/omega/{pass,fail}/facts/foreign_scalar_record_result_through_*`.
The root repeats the local witness. The const place names the C construction
inputs: a real foreign operation is not a constant initializer.

The existing `providers/foreign_multiregister_result_exit` exposes the baseline:
C returns the correct native fields, while Terminal declines the absent trusted
binding. This row adds no replacement C implementation or source-name dispatch.
Omega's adapter admits the complete normalized libc binding, selected requirement,
ordered scalar signature, public two-i64 record shape and evaluated calling
application. Runtime-ABI invokes actual C `lldiv`; denominator zero and the
unrepresentable `i64::MIN / -1` pair refuse before the call.

Terminal publishes actual scalar contents into its ordinary fresh placed record
storage. The scalar sum-case path uses the same carrier/publication and retains
its real discriminator; a record has none. Complete ordered field IDs, types and
actual value restrictions are checked before publication. The host supplies no
opaque storage identity, reference backing, claim, domain qualification or
projected evidence. Copies and moves then use ordinary record custody.

All four hosted entries are declared. The selected C record is 16 bytes:
macOS ARM64 and Linux x86-64 return it in two registers; Windows x64 uses
caller-owned result memory, shifts the two scalar inputs and returns that
result pointer. The Windows binding is the exact
`api-ms-win-crt-utility-l1-1-0.dll` / `lldiv` import from Microsoft's x64 UCRT
SDK import library, forwarding to the actual `ucrtbase.lldiv` export. Linux
uses `libc.so.6` with the selected architecture's symbol version; macOS uses
`/usr/lib/libSystem.B.dylib` / `_lldiv`. Matching-host observations, producer
identities and retained products are recorded separately in qualification/PR
evidence; declarations and cross-target builds provide no runtime credit.
Linux ARM64 remains unmeasured until a matching host runs the row.

Matching-host evidence for this row: optimized Mac producer `2cffebee3b86`
passed all nine pinned positives and rejects plus the root and nine ordinary
public `run --both` routes. Hosted run
[40201214950](https://github.com/CathedralOS/Omega/actions/runs/40201214950)
built producer `ffb3227a24d6` on Linux x86-64 and Windows x64: each host passed
nine positives with native/Terminal exit zero and agreement, and nine exact
`holds no value` rejects. Windows also resolved the named API-set export to
the loaded UCRT image and called both signed input pairs. The retained ZIPs,
producer profiles, native/Terminal products and original reports are bound
in the scoped qualification evidence. These observations keep their actual
producer identities after documentation and golden-only commits.

This advances real Windows indirect C results when qualified there. It does
not qualify a macOS indirect C result (this pair is register-returned), borrowed
descriptor arguments, stack-slot overflow, or the parent's complete host matrix.
The separate `aggregate_struct_return_abi` Quartet is Omega-produced and cannot
substitute for an actual C return.

## Direct foreign production

The same fact also needs the concrete foreign-call production, independently of
the selected `IntegerDivision::divide` route. The [direct root](direct/main.omg)
and its nine places call `divide` directly for both original input pairs. They
retain the whole-record double use, the earlier result across the second call,
and all six signed-field comparisons. The exact `lldiv` bindings and public
two-`i64` carrier remain unchanged; each external leaf uses `boundary machine`.
Nested helpers use ordinary `machine` declarations and explicit arguments.

The direct places are [local](direct/local/main.omg),
[field](direct/field/main.omg), [parameter](direct/parameter/main.omg),
[call result](direct/call_result/main.omg),
[match join](direct/match_join/main.omg),
[const name](direct/const_name/main.omg),
[nested machine](direct/other_state/main.omg),
[other machine](direct/other_machine/main.omg), and
[target body](direct/target_body/main.omg). Every place has an `invalid/main.omg`
that removes only `Division`'s `[copy]` property and retains the expected
`holds no value` rejection. Exact copies are pinned under
`tests/omega/{pass,fail}/facts/foreign_scalar_record_result_direct_through_*`.
Every positive declares all four hosted entries and `Expected exit: 0`.

This is another production of the existing NORMALIZED-ABI-LOWERING requirement,
not another fact or a larger-count ABI row. The applicable requirement layers
below apply to it as well. Optimized producer `22a3c05942f34c25d81ff118cbb674ad8f47785a` now executes the
root and all nine direct places with `omega run --both` on `macos_arm64`:
native exit 0, Terminal exit 0, and successful `inspect-terminal --artifact`.
All nine copy-removal twins reject with `holds no value`. The exact signed
inputs, both result fields, earlier-result copies and consuming operation remain
unchanged. The match join additionally observes each arm's marker so an inverted
branch traps.

Fresh fixed baseline `14d6fefba285962378d30e5992de83fde81b3c69` reproduced missing
applications for direct calls and an incorrect requirement-call projection inside
target bodies. The repair retains each declared callable and the exact receiving
foreign row's original admitted application; it preserves nominal parameter
identities, target, plan and full commitment without selecting unused providers.
The original large-frame scalar fixture again agrees at exit 70; the new scalar
target-body control agrees at exit 0. The affected comparison has zero lost
outcomes in 42 source records and 24 native/Terminal records (22 observed
agreements, one source-only callback fixture, and one inherited table-lowering
refusal). The scoped Rust suite records 60 passes and three inherited failures;
all three reproduce on the fixed baseline with identical assertion payloads and
source-guard violation lists after line-number normalization.

Other hosts have authored entries and bindings but receive no execution credit
from this macOS run. Remaining foreign ABI/backing acceptance stays on
NORMALIZED-ABI-LOWERING. See [the direct run record](direct-qualification.json).
The historical selected-call observations above remain separate evidence.

Applicable requirement layers:

- Establishment/identity: exact foreign admission and complete field validation
  publish a fresh owned occurrence; the first and later results remain distinct.
- Composition: all nine transports preserve the same fields and copy obligation;
  both match paths and explicit target bodies consume the same operation.
- Soundness: the noncopy twins reject before execution; unavailable bindings and
  unsupported authority refuse. Signed input/field widths remain exact.
- Psi 05–07: actual fields use ordinary record cells, copies and state arrivals;
  existing sum-case discriminators retain their separate field namespace.
- Omega 00–02: the exact normalized C application retains ordered formals,
  complete result geometry and validated ABI placements.
- Omega 03–06: caller homes and live-across-call transport preserve the first
  result, including Windows' hidden result pointer and subsequent scalar inputs.
- Omega 07–09: real matching-host images resolve the admitted libc import and
  execute against Terminal. No unobserved target receives engine credit.

```sh
omega run --both samples/cli/facts/foreign_scalar_record_result/local/main.omg
omega --check samples/cli/facts/foreign_scalar_record_result/local/invalid/main.omg
python3 tools/ci/compiler_foreign_records.py
```

## Build-owned native requirements

The current repair owns **BUILD-OWNED-NATIVE-BINDINGS**. It transports the same
quotient/remainder record fact through the row's existing nine places; it does
not add another numeric fact or replace libc `lldiv`. The package Build binds
`IntegerDivision::divide` directly to one evaluated target and a typed
`ForeignBinding` locator. No bodyless per-symbol forwarding machine or `via`
clause supplies that binding.

The fixed baseline `4c6b385bb20cb8661455c6fddb4efc130106c5fc` refused the
new direct Build form. The focused constructor baseline also refused the
uncounted typed locator. Retained candidate attempts remain separate records
under `build/native-binding-evidence`; none is a successful runtime observation
unless its actual native and Terminal reports say so. The historical evidence
elsewhere in this README retains its original producer and source identity.

The [applied requirement control](controls/exact_requirement_application/main.omg)
keeps the same two signed C calls, all six field/copy checks, and result carrier,
while selecting `IntegerDivision<u64>::divide`. Its
[publication-refusal twin](controls/exact_requirement_application/invalid/main.omg) changes
only the Build selection to `IntegerDivision<u32>::divide`. It remains Source-valid;
its build-only corpus pin requires native publication to refuse the missing
`u64` provider, rather than inventing a Source rejection. The otherwise
unused nominal argument must retain its identity; identical physical C layouts
do not permit that selection to satisfy the `u64` call. The baseline producer
`1afbd5b7cc9e4b5cf30cc7d3aa7e129dee7c798a` refused both at the qualified
application syntax. That refusal is preserved, rather than reported as the
twin's final intended application mismatch.

The applicable acceptance is:

- Establishment: a reached operation on the original Build activation retains
  its exact designated requirement, evaluated target and closed locator. Ordinary
  fixed-byte constructor inference supplies locator lengths; explicit bad lengths
  remain formation errors. A locator supplies no foreign behavior proof.
- Identity and lifetime: original source/package custody and complete static
  applications survive Product discovery. Trait and member telescopes remain
  separate, including unused type/const operands and captured versus fresh
  lifetimes. Unused designated operands still receive ordinary formation checks.
- Composition: one canonical executed request ledger supplies the ordinary
  provider selector, including package-private raw slots, defaults, explicit
  overrides, complete schema coverage and duplicate/conflicting-row refusals.
  A Build-host provider is not installed by a Product native request.
- Soundness: only admitted normalized locator/calling applications can reach a
  loader. Missing/ambiguous/inaccessible selections refuse; checked adaptation
  remains an ordinary implementation, not an asserted locator conversion.
- Psi 05–07: the issued request travels through checked Build calls and both
  execution engines without an AST replay, synthetic foreign machine or
  reconstructed source identity. The selected requirement retains its declared
  full signature and loans.
- Omega 00–02: exact target/ABI compatibility, full calling convention, generic
  schema and independently declared same-stack demand are retained by the
  derived native candidate. Typed table fields keep their separate explicit
  source operation; they are not new `ForeignBinding` cases.
- Omega 03–06: the existing record result/copy and live-across-call obligations
  remain required, including the physical result-memory lane on applicable
  hosts. The migration grants no new activation bound.
- Omega 07–09: the same real C symbol and admitted relocations must execute and
  agree in native and Terminal engines on each measured host. Authored entries
  and cross-target compilation provide no foreign-host execution credit.

Fresh checking, intended reject diagnostics, native/Terminal agreement, canonical
reloads, exact pins and final producer/source applicability remain required for
closure. Constructor syntax, an intermediate compiler check, or an issued
request alone does not complete the row or its parent task.

The [package-owned producer](package_owned/main.omg) uses a dependency's private
raw requirement and direct native binding. Its public checked implementation
exports the original record fact; the application neither imports the private
raw slot nor repeats its locator. The [copy twin](package_owned/invalid/main.omg)
removes only the record's copy permission. The
[missing monomorphic member control](controls/package_missing_member/main.omg)
adds a second raw member without a binding: every monomorphic schema member
still needs coverage, even when the application never calls that member.

[Locator constructor controls](controls/locator_inference/) check inferred
nested Mach-O/PE lengths, the zero unused coordinate prescribed by the sealed
locator schema, ordinary record inference and explicit concrete context.
Their invalid controls retain incompatible used widths and unresolved ordinary
user coordinates; no rule supplies arbitrary zero parameters to user data.
The [unused const application](controls/unused_requirement_const/main.omg)
retains an ordinarily formed `42` tuple even when no operation demands it; its
[twin](controls/unused_requirement_const/invalid/main.omg) contains a zero-divisor
formation error, which an unused selection must not erase.

Recorded intermediate observations: producer `e0d52193cb1b` executed the root
and nine direct places with native/Terminal agreement and reloaded their ten
canonical artifacts. Producer `842ec84f27fb` executed the private dependency
producer with agreement; its copy and incomplete-schema controls refused.
Those are retained observations of their exact producers, not final-candidate
qualification or unmeasured Windows/Linux execution. Full migrated-library/audio execution, final replay and independent review
remain part of this task's acceptance.


The [generic Build helper](controls/generic_requirement_helper/main.omg) keeps
this same C fact while an ordinary `install<Tag>` helper transports the exact
closed requirement and inferred locator. Its [twin](controls/generic_requirement_helper/invalid/main.omg)
changes only the installed nominal type. The
[qualified type control](controls/qualified_requirement_type/main.omg) distinguishes
same-spelled declarations in separate ordinary modules; its
[twin](controls/qualified_requirement_type/invalid/main.omg) selects the other type.
Both twins are Source-valid native-publication refusals, not Source errors.

The [owned literal controls](controls/owned_byte_literals/) cover the evaluator
snapshot required by those locators: exact fixed bytes, qualified byte elements,
and the empty array. Bad element predicates also reject inside an unused generic
body, and incorrect widths reject before publication. These are constructor
prerequisites, not additional numeric rows. The
[named terminal-call control](controls/named_terminal_call/main.omg) retains the
same C operation through a returning terminal call; its copy twin still rejects.

The [invocation-roster control](controls/invocation_clause_rosters/main.omg) keeps
reach and synchronous invocation identities separate when their source clauses
interleave. Its two controls remove one invocation edge from the body or boundary
signature. This pins the parser arena-contiguity prerequisite exposed by the
migrated standard library; it adds no new source syntax or implicit invocation.

Producer `ace8ceebd74a` executed the unchanged generic helper and the qualified,
closed-application, unused-const and private-package controls with native/Terminal
agreement, then reloaded all five canonical artifacts. The owned-literal
compatibility controls also agreed; malformed element and width controls retained
their Source refusals. These are intermediate exact-source observations.
Migrated-library/audio execution, current-main integration, final qualification
and independent review remain required for the parent outcome.

The [Named callback forwarding control](controls/named_callback_forwarding/main.omg) retains an explicit machine callback through an ordinary Named wrapper, with two distinct callback outcomes. The invocation roster controls independently reject a missing body or requirement ceiling.
