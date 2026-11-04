# Returning a borrowed byte view through a runtime function choice

A runtime-selected function returns a view of bytes owned by its caller. These
programs check that the view still reads the caller's data after the record
holding it passes through each place below. A byte view carries an address and
a length; those describe the original array, not temporary storage inside the
function that returns it.

Fact: a finite-family method on a borrowed record returns a descriptor of the original backing, and reading its first byte yields 11.

**BUILD-OWNED-TARGET-SELECTION** owns moving the remaining platform-specific
helper selection into Build. **FINITE-GENERIC-DISPATCH** owns preserving the
public input-to-result borrowing contract through runtime function selection.
Both parent outcomes remain open.

The original array is `[11, 22, 33]`. Every transport place also carries
`[11, 44, 55]`. Both views must still begin with 11; their different trailing
bytes make choosing the wrong array observable. Each uses the original width
arguments 16 and 32. The trailing-byte expectation is a runtime check only;
it supplies no proof or borrowing permission. The length check makes both byte
reads safe independently of the returned reference's lifetime.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Both match choices run with independently supplied expected bytes, so always
choosing one arm traps. The field example checks the neighboring values 17 and
23 around its stored views. Named arrays are copied into owned runtime storage
before borrowing them. The `other_state` folder remains the nested-machine place.

Twelve lifetime twins change only `Header::window` to return invocation-local
storage. The replacement and saved-pointer twins instead install a reference to
local storage at their replacement store. Each changes only the borrowing source
and must reject because that storage does not outlive the call. Every sample's Main and Build
files match its fixture under `tests/omega/{pass,fail}/facts/finite_family_returned_descriptor/`.
All positives bind the four hosted targets and declare exit 0.

The [stored-pointer example](pointer_field/main.omg) carries a reference to a
single caller-owned byte. Its [twin](pointer_field/invalid/main.omg) substitutes
invocation-local storage. The [replacement example](replaced_source/main.omg)
changes a stored reference from the caller's byte 29 to its byte 11, returns the
new reference, and checks that both the stored and returned references read 11.
Its [twin](replaced_source/invalid/main.omg) installs invocation-local storage.
These are additional productions of the same borrowing requirement.

The [caller-view example](caller_view/main.omg) forms a checked `Packet` view
of a caller-owned `BackingPacket` before forwarding it. Their byte fields have
different names. Both width choices must return the original byte 11, and the
backing record must still read 11. A third call uses a separate byte 23, so
reusing the first pointer or inventing a constant result fails. Its
[twin](caller_view/invalid/main.omg)
changes only the selected method to return invocation-local storage.

The [incoming-pointer join](incoming_join/main.omg) chooses between an unchanged
input pointer and a new borrow of a field before the same selected call. Both
choices run at widths 16 and 32. The input reads 11 and the field reads 23, so
losing either predecessor or choosing the wrong pointer fails. A second helper
joins two unchanged input pointers and also exercises both choices. Its
[twin](incoming_join/invalid/main.omg) changes only the selected method to return
local storage. This checks that preserving a view also preserves ordinary
incoming pointers in the complete set of possible sources.

The [saved-pointer replacement](saved_source/main.omg) saves a mutable pointer,
then changes the reference stored in its record before the selected call. The
returned and stored references must both read the new byte 11, rather than the
old byte 29. This distinguishes the saved pointer's identity from the record's
later contents. Its [twin](saved_source/invalid/main.omg) installs local storage
instead.

The [target-qualification example](target_qualification/main.omg) calls an
ordinary `SelectedTarget::target_only` requirement from a selected family member.
Its public lifetime explicitly relates the returned view to its input. Four
Build rows select the same identity implementation because the original four
bodies all returned their input unchanged. The [missing-provider control](target_qualification/invalid/main.omg)
has identical source and supplies only the Linux ARM64 row. Its source is valid;
Linux ARM64 must execute with exit 0, and the other three hosted targets must
refuse missing supply during native publication. Its build-only pin is
`pass/facts/finite_family_returned_descriptor/target_qualification_missing_supply`.
Target-specific refusal and exit files state those separate outcomes.

The main `target_body` place also selects one ordinary forwarding implementation
through explicit Build rows. Its requirement has empty service reach; the entry
names its synchronous call with `invokes SelectedForward`. The qualification
example similarly publishes `invokes SelectedTarget` on `Windowing::window`.
Calling a boundary requirement does not itself add a service to its reach.
The existing optional macOS x86-64 entry remains in the target place.

These programs make incorrect data transport observable: both match inputs run,
the two arrays have different trailing bytes, and replacements must return the
new byte. Widths 16 and 32 have the same returned-data behavior. Which identical
specialization executes, selection between implementations with different
behavior, and dispatch-table relocations remain outside this row's evidence.

The governing contracts are [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary),
[dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families),
[returned views](../../../../wiki/spec/language/lifetimes.md#returned-views),
[Build-owned target selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection)
and [service reach and synchronous calls](../../../../wiki/spec/language/effects.md).

The evidence must establish the following obligations:

| Requirement | What these programs check |
| --- | --- |
| Establishment | Construction and entry contracts supply the actual data, access and width choices. A length check makes indexing safe; it does not create a lifetime. |
| Identity and lifetime | The two arrays keep their distinct trailing bytes. Whole-record and saved-pointer writes replace byte 29 with byte 11 while both stored and returned references remain usable. Each local-storage twin must reject. |
| Composition | Calls substitute the actual selected input. Both pointer-join choices run, and an existing checked view retains its field interpretation through the call. A later caller equality check supplies no borrowing permission. |
| Ownership and proof | The public requirement owns the returned-reference contract. Local provider variables cannot replace public inputs. Shared references may coexist; live-borrow controls still forbid conflicting writes. |
| Portable program | Terminal Psi must preserve the actual data arguments, returned views and explicit cleanup through the selected call. A reference installed into caller storage survives the returning call; a genuine local parent still needs disposal. Broader proof-erasure and crash-route acceptance remains open. |
| Build, layout and calling conventions | Explicit Build rows select ordinary implementations for the hosted targets. Address, length and stored-pointer fields cross calls and returns. Missing supply must refuse publication on exactly the unsupplied targets. |
| Registers and call storage | Calls preserve both view components and the neighboring values 17 and 23. The interpreter and native result must agree. This row does not establish explicit activation bounds or every arithmetic instruction. |
| Native images | Matching-host execution must read the authored bytes through the selected calls and emitted image. Dispatch-table relocations, long-branch relaxation, callbacks and other image mechanisms retain their own acceptance. |

## Current validation

All 15 positive examples have actual native/Terminal exit-zero agreements on
Linux x86-64 at the retained compiler revisions `3baa68a0860` and
`0c53c5e5b938`. All 14 local-storage twins reject. The
[ordinary CLI replay](https://github.com/CathedralOS/Omega/actions/runs/41785423833)
at `3baa68a0860` passes all 30 outcomes: 15 exit-zero agreements and 15 intended
refusals, including missing target supply. Those observations retain their
original compiler and input identities.

The shared-slice regression required one further repair. The
[failed trace](https://github.com/CathedralOS/Omega/actions/runs/41808313137)
shows that loan matching discarded the checked result's region extent.
Compiler `845a43cb4fe` preserves that extent and matches the enclosing collection
of the actual opaque window, retaining access and fixed-coordinate checks.
The unchanged slice program now returns its authored 70 on both engines through
[ordinary CLI package review and execution](https://github.com/CathedralOS/Omega/actions/runs/41813706580).
The replay uses the retained optimized compiler and verifies 179 original
program/library inputs. Its earlier direct-fixture command remains failed on
an unaccepted program-entry binding; it ran neither engine.

[Integrated validation](https://github.com/CathedralOS/Omega/actions/runs/41815976095)
at `059fb0b41d8` passes all 49 Source cases and records 24 actual exit-zero
agreements, including all 15 fact examples, plus the intended supply refusal.
That report remains failed because its additional slice corpus run timed out
at 120 seconds; the public stage did not run.

The [same-compiler replay](https://github.com/CathedralOS/Omega/actions/runs/41822182167)
completes those missing observations without a rebuild. With one corpus worker
and the unchanged limit, the slice agrees at 70 in 72 seconds. The ordinary fact
commands produce all 15 exit-zero agreements and 15 intended refusals; ordinary
slice package review and execution also agree at 70. The audit verifies 451
distinct source/oracle/library inputs and the original retained binary profiles.
The earlier timeout remains a failed observation, not a passing report.

The same fixed compiler `059fb0b41d8` completes the fact on
[Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41823218447),
[macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/41823222419) and
[Windows x86-64](https://github.com/CathedralOS/Omega/actions/runs/41823226523).
Each host checks all 14 local-storage twins and executes all 15 positive examples
with native and Terminal exits of 0 through both corpus and ordinary commands.
The missing-provider control also executes at 0 on its supplied Linux ARM64
target; macOS and Windows refuse publication as authored. Each saved report,
optimized producer profile and all 274 frozen inputs have been verified.
Together with Linux x86-64, these observations complete this borrowing fact
across the four hosted targets; the parent outcomes remain open.

The earlier failed corpus reports remain retained. Separate case-descriptor
and nested-view programs also refuse on published compiler `9566f10e0d29`.
Their early borrowing/formation errors provide no intended enforcement or
execution evidence, and those gaps remain open. The unsuccessful lookup-scoping
attempt was reverted after review found valid nested results it could exclude.

The original replacement program's lowering failure is retained in
[41575232529](https://github.com/CathedralOS/Omega/actions/runs/41575232529).
The [cleanup trace](https://github.com/CathedralOS/Omega/actions/runs/41778481192)
establishes the later storage error: a transfer moves the reference into caller
storage while preserving its creation identity. The repair checks its current
storage and still follows every parent and lifetime obligation. Local parents
and siblings must close through their actual cleanup.

Exact producer identities, input hashes, raw observations, earlier source and
independent reviews remain under `build/finite-descriptor-selected-20261103/`.
Both parent requirements stay open. This row does not establish the remaining
generic dispatch, proof-erasure, crash-route or product-compiler acceptance.
No language-design decision is needed.
