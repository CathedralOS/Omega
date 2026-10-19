# Target-scoped constant realization

Owning requirement: TARGET-SEMANTIC-APPLICATIONS. An existing pure terminating
boundary-trait provider supplies the literal extent of a record; the selected
target closes that extent before the record layout is observed. Linux x86-64
and Linux ARM64 select 4; Windows x86-64 and macOS ARM64 select 8. Each explicit
single-target compilation must use its own selected realization. No target
projection API is introduced.

Frozen baseline: 53cd2938437a29356a48d6eaf29f14de07a4bb99. The unchanged main.omg
hash is 78888b33cf489be33ae364dabc99945e647902c148df98d5d7a89a673333c9be.
A freshly built compiler (optimization level 3, SHA256
8bd21075a7ba6df382404d72fe8da6ca4ac9a02a5f34f915f4f19d6bf9d37e07)
rejected --check --target linux_x86_64 in 13.933 seconds:
`build-time call from limit has no exact target-machine identity`.
The primary source and its 0 exit oracle are retained across repair.

Host acceptance command: `omega run --both main.omg` (no explicit target).
An explicit `--target` produces target artifacts and does not execute them.
The compiler now defers the exact named boundary-trait requirement, validates the
whole selected conformance, closes the extent before querying its layout, and
retains the selected realization after folding. A canonical Terminal artifact
carries the exact requirement, selected declaration/body, normalized plan
commitment and target scope. Receiving Terminal and native admission reconstruct
those commitments from complete canonical plan inputs. This historical closure
grants no runtime provider installation or authority.

The unchanged primary source executes with native exit 0 and Terminal exit 0 on
Linux x86-64. Independent explicit Linux and Windows compilations retain integer
constants 4 and 8 respectively after canonical encode/decode. Windows artifacts
are interpreted and admitted for native realization on this Linux host; Windows
native execution is not asserted. Current build/configuration rules still require
provider-derived constants to agree within one multi-target compilation.

The source controls remove only the Linux selection, remove an otherwise unused
member of the selected whole conformance, or give the selected provider a false
postcondition. The Build inputs match each paired positive exactly; the fail
corpus uses the first authored target, Linux x86-64. They reject for ambiguous
unselected providers, incomplete conformance and unproved ensures respectively.
A multi-method provider challenges method-row selection and keeps an unused
crashing first sibling outside evaluation admission. Selecting the pure second
method challenges accidental first-row lookup. A zero-extent provider distinguishes
a completed zero layout from a still-unresolved layout. Their native and Terminal
exits are 0.

A source-backed public API replay compiles through the ordinary package route,
roundtrips canonical Terminal bytes, then checks receiving and native admission:

| Control | Required result |
| --- | --- |
| Original exact plan inputs and target | Admit; Terminal completes |
| Equal-valued `Same` provider in place of `Small` | Reject exact realization mismatch |
| Absent provider/closure | Reject missing historical closure |
| Copied digest in place of canonical plan input | Reject reconstructed commitment mismatch |
| Repeated plan inputs | Reject incomplete or repeated closure |
| Linux artifact reused under Linux ARM64 (also 4) or Windows | Reject target closure mismatch |
| Provider-free folded extent | Execute without historical provider closure |

No arbitrary dependency struct or copied digest can mint the sealed admission
token. The source-backed controls use genuine provider plans/candidates; they do
not fabricate checked evidence or weaken authority, custody or termination.

Governing authority: [evaluation](../../../../wiki/spec/language/evaluation.md)
requires same selected-provider/target value semantics and retention of selected
realization dependencies even for literal-only code;
[provider selection](../../../../wiki/spec/build/provider_selection.md) admits
ordinary pure literal-returning providers;
[Terminal product](../../../../wiki/spec/terminal-psi/product.md) distinguishes
historical folding compatibility from runtime choice; and
[boundary calls](../../../../wiki/spec/terminal-psi/boundary_calls.md) keeps absent
trait telescopes distinct from canonical empty operator applications.

The witness consumes the constant through a helper call, array extent, record
field layout, and target-selected runtime body. Target/application compatibility
is the mechanism under test; repetition through unrelated numeric transport
places is not additional evidence. The broader task's target projection surface,
package generic application closure, supplemental intrinsic-plan reconstruction,
provider-free target-observation dependencies and other constant consumers stay
open. This row establishes the checked-provider mechanism, not all of
TARGET-SEMANTIC-APPLICATIONS.
