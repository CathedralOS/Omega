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

Each lifetime twin changes only `Header::window`: it returns a view of an array
local to that invocation instead of the caller's array. It must still reject
because that storage does not outlive the call. Every sample's Main and Build
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

The first probe put references inside both records. Fixed-base run
[41685737527](https://github.com/CathedralOS/Omega/actions/runs/41685737527)
refused that cast before the call, as did candidate run
[41685737741](https://github.com/CathedralOS/Omega/actions/runs/41685737741).
It did not test returned-view composition. This probe uses supported scalar
records to isolate the caller's existing view; it retains the same byte, width
choices and local-storage twin. The earlier source remains at `34f4d51`.
No passing result is claimed for the new probe yet.

The [incoming-pointer join](incoming_join/main.omg) chooses between an unchanged
input pointer and a new borrow of a field before the same selected call. Both
choices run at widths 16 and 32. The input reads 11 and the field reads 23, so
losing either predecessor or choosing the wrong pointer fails. A second helper
joins two unchanged input pointers and also exercises both choices. Its
[twin](incoming_join/invalid/main.omg) changes only the selected method to return
local storage. This checks that preserving a view also preserves ordinary
incoming pointers in the complete set of possible sources. Fresh results are
pending.

The [saved-pointer replacement](saved_source/main.omg) saves a mutable pointer,
then changes the reference stored in its record before the selected call. The
returned and stored references must both read the new byte 11, rather than the
old byte 29. This distinguishes the saved pointer's identity from the record's
later contents. Its [twin](saved_source/invalid/main.omg) installs local storage
instead. Fresh results are pending.

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

The governing contracts are [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary),
[dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families),
[returned views](../../../../wiki/spec/language/lifetimes.md#returned-views),
[Build-owned target selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection)
and [service reach and synchronous calls](../../../../wiki/spec/language/effects.md).

The complete acceptance requires the following:

- Checked construction establishes each incoming view and its access. A lifetime
  name or matching field name cannot create a reference or permission.
- Returning a view preserves every reference it contains and its relationship to
  the caller's backing. Replacing one stored reference retires that old reference
  without changing an unrelated one. Both resulting shared references may coexist;
  an exclusive reference still cannot be duplicated.
- Calls and joins preserve the actual selected input. The caller's later length
  and byte checks cannot justify returning storage that has already died.
- The public requirement owns the borrowed result contract. A private provider's
  local variables cannot stand in for public input coordinates, and every family
  row must have valid target supply.
- Terminal Psi and native execution must both read the caller's original storage,
  including its correct address and length. Successful checking or image emission
  alone does not establish that result.

## Current validation

The unchanged replacement program fails during lowering on fixed baseline
`0d2886a26e6b17ede96f1accbb6501a2a85fbd30`
([run](https://github.com/CathedralOS/Omega/actions/runs/41575232529)).
The repair at `a251628c888a10e0f2871d4b15a1429cf1746d53`
([run](https://github.com/CathedralOS/Omega/actions/runs/41668616576)) executes
that program and the stored-pointer program with native and Terminal exit 0
through both corpus and public sample commands. Their local-storage twins reject.
Those observations cover the two examples. The complete row still needs current
validation.

At `750caced8cc6b894273004bf2ecb2d94ec57ed3c`, all twelve original fact programs
execute with native and Terminal exit 0 on
[Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41754288583)
and [Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41754324372).
Every measured local-storage control rejects. The missing-supply control refuses
publication on Linux x86-64 and executes on its supplied ARM target. The source
leg selects macOS ARM64 independently and also observes its missing-supply refusal.
The reports remain failed because the new caller-view example still loses its
input origins. Neither report reaches public sample execution. Linux also has a
slice-return refusal whose current-main attribution remains unresolved.

The next probe at `faa7d0645db43495ece24190cc3f6def9d489942`
([run](https://github.com/CathedralOS/Omega/actions/runs/41759230903)) still refuses
the caller-view and incoming-pointer join. Its stored-pointer, replacement and
existing view-composition controls execute with exit 0 in both engines; all six
intended source controls reject. These partial results do not establish the new
examples.

Current source separates the captured pointer from the callee's later read of
its contents. The pointer keeps its original evaluated value. When the callee
reads data through that pointer, it uses storage after every argument has
completed, including effects from later arguments. Independent source review accepted
this correction. Fresh execution of the caller-view, incoming-pointer join and
saved-pointer replacement, followed by all public fact projects, remains open.
The preceding `e46c33e3c75a055c41e48fbc53c7ead8c2548e8b` probe
([run](https://github.com/CathedralOS/Omega/actions/runs/41763942681)) checks all
three new examples, but native legalization refuses their storage relationships.
The stored-pointer and existing view-composition programs execute with exit 0 in
both engines. The original replacement also refuses lowering at that source.
An existing case-descriptor example rejects identically on the earlier `750`
compiler, confirmed by [retained baseline run](https://github.com/CathedralOS/Omega/actions/runs/41766271632).
Its invalid control also has an unrelated diagnostic, so it does not establish
enforcement of that obligation. These failures remain open.

Actual producer identities, raw reports and earlier source are retained under
`build/finite-descriptor-selected-20261103/`.

Windows x86-64 and macOS ARM64 native execution remain unverified for this
change. Broader generic dispatch, cleanup and product-compiler acceptance are
not established by this row. Both parent requirements remain open. No owner
design decision is needed.
