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

The [merged-source Linux run](https://github.com/CathedralOS/Omega/actions/runs/41829998724)
at compiler `9ad31e5e871` passes all 49 checking cases. It records 24 actual
native/Terminal exit-zero agreements, the unchanged shared-slice program at
70/70, and the intended missing-provider refusal. Ordinary commands pass all
15 fact examples at 0/0 and all 15 intended refusals. The reports, optimized
compiler profiles, 274 frozen inputs and 23 unchanged repair files are verified.

The complete fact also passes on
[Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41823218447),
[macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/41823222419) and
[Windows x86-64](https://github.com/CathedralOS/Omega/actions/runs/41823226523)
at compiler `059fb0b41d8`. Each checks all 14 local-storage twins and executes
all 15 positive examples at 0/0 through corpus and ordinary commands. The
missing-provider control also executes at 0/0 on its supplied Linux ARM64
target; macOS and Windows refuse publication as authored. Independent review
accepts those results for this merge: the repair and programs are unchanged,
Linux freshly checks the changed shared consumers, and the target changes
concern atomic and segment operations absent from these programs. These remain
actual `059fb0b41d8` observations, not execution of `9ad31e5e871` on those hosts.

The earlier [integrated Linux report](https://github.com/CathedralOS/Omega/actions/runs/41815976095)
at `059fb0b41d8` remains failed because its additional slice corpus case timed
out at 120 seconds; the ordinary stage was skipped. The
[same-compiler replay](https://github.com/CathedralOS/Omega/actions/runs/41822182167)
completes that slice at 70/70 in 72 seconds with one worker under the unchanged
limit, all 30 ordinary fact outcomes, and ordinary slice review/execution at
70/70. The audit verifies 451 distinct inputs. Neither the later replay nor the
fresh merged-source success changes the earlier failed result.

The original replacement program's lowering failure is retained in
[41575232529](https://github.com/CathedralOS/Omega/actions/runs/41575232529).
The [cleanup trace](https://github.com/CathedralOS/Omega/actions/runs/41778481192)
shows a reference transferred into caller storage while keeping its creation
identity. The repair follows its current storage and every parent and lifetime
obligation. A separate [failed trace](https://github.com/CathedralOS/Omega/actions/runs/41808313137)
shows forwarded slice matching discarding the checked region extent; the repair
preserves that extent without inventing an exact address. Local parents and
siblings still require their actual cleanup.

Separate case-descriptor and nested-view programs also refuse on published
compiler `9566f10e0d29`. Their early borrowing/formation errors supply no intended
enforcement or execution evidence, and those gaps remain open. The unsuccessful
lookup-scoping attempt was reverted because it excluded valid nested results.

This row establishes the public input-to-result borrowing relation for the
shared byte views and Packet examples above. `FINITE-GENERIC-DISPATCH` retains
field reads under entry content claims, target/const qualification and its full
remaining acceptance. `BUILD-OWNED-TARGET-SELECTION` also stays open. Broader
generic dispatch, proof erasure, crash routes and product-compiler acceptance
are not established here. No language-design decision is needed.

Exact producer identities, input hashes, raw observations, retained source and
independent reviews are under `build/finite-descriptor-selected-20261103/`.
