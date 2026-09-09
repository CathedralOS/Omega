# Terminal interpreter

Start at [lib.rs](src/lib.rs). Execution follows verified
[calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
and [structural ownership](../../../../wiki/spec/terminal-psi/ownership.md).

## Cyclic execution

Ordinary interpretation accepts verified natural-ranked scalar, plain-owned-input,
immutable-view, receiver, and primitive-local
graphs under the [control contract](../../../../wiki/spec/terminal-psi/control_flow.md).
The legacy one-machine structural Unit countdown uses its own verifier entrance:
it reconstructs `0 < remaining` as the unsigned `1 <= remaining` premise and
checks exact subtraction before creating resumable state. This interpreter
acceptance grants no native, fixed-fuel, provider-installation, or mixed-work
authority.

[block_bindings.rs](src/block_bindings.rs) captures all selected operands before
cleanup and simultaneous successor installation. Affine inputs move once;
Unrestricted descriptors remain reusable. Rebinding changes the descriptor place,
not its referent or field backing. Missing/duplicate owners and transfer/discard
overlap reject before mutation. Fuel suspension preserves the uncommitted edge.
This whole plain-owned route does not admit partial/qualified owners. Exact
unqualified mutable byte-view block parameters retain an existing field loan;
other mutable borrowed block parameters remain unsupported.

## Calls and work

The interpreter charges a call before entering its owned frame. Exhaustion inside
the callee resumes without paying or invoking that call again. A callee crash
retains its original site and edge charge, with no fabricated caller crash.
Ordinary recursive call graphs remain fenced until their tail/ranking evidence
is admitted; cyclic block execution has its separate verified route above.
Fixed-work composition distinguishes normal return bounds from crash bounds:
only a normal return composes the caller's remaining work.

## Primitive local storage

[primitive_storage.rs](src/primitive_storage.rs) establishes fresh referents on
each activation or loop reentry. Borrowed calls share their backing; reads copy
the current scalar, so earlier snapshots survive later writes. Suspension retains
the storage, and departure reclaims it without affine cleanup. Local identities
cannot collide with declared places or supplied host identities. This support
does not provide native allocation or permit owned local escape; see
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md).

## Bounded byte fields

Whole-field replacement supplies live backing associated with the original
structural referent, carrier path, and field. Field-length observations and
indexed byte stores retain that backing across calls and fuel suspension.
Indexed stores validate the current extent before mutation and detach shared
immutable backing; they cannot change source literals or sibling fields.
An opaque incoming structural identity does not supply initialized field bytes:
measuring or indexing an unprovided field fails rather than assuming empty
or zero-initialized storage. This execution support does not establish native
byte-field realization.

The current migration path lets an initialized bounded byte field supply an
unqualified mutable boundary parameter. The boundary-specific resolver
retains the original referent, complete record/array path, and capacity; it does
not make bounded storage generally interchangeable with views. The exact
mutable field-to-byte-view presentation is also available to ordinary Unit
helpers on the verifier's admitted field-only paths; it retains the same field
loan rather than copying or owning bytes. Missing backing still rejects.
`TerminalEffectHandler::handle_effect_with_byte_buffers` receives the pre-call
bytes and stages whole live-sequence replacements through
`TerminalBoundaryByteBuffer::replace`. Oversized replacements reject without
changing the staged value. All buffers and the declared response must validate
before any field writeback or completion is committed. The effect trace records
pre-call bytes; `structural_byte_sequence_field` observes committed replacements.
Handlers without this callback reject mutable buffers before performing an
effect. Missing backing remains an error, including for an opaque entry object.

Installed checked providers receive a frame-owned mutable field binding, not a
copy of the incoming bytes. Whole mutable-view arguments can forward that binding
through ordinary helpers or reborrow it at another boundary. Each external call
observes the current backing and commits to the original field; returning from
the provider does not restore its entry snapshot. Calls and fuel suspension
retain the binding's exact referent, record/array path, and capacity. An equal
opaque identity without that binding supplies no mutable loan. Byte reads and
subslice operations still require immutable bindings. Native byte-field forwarding and
`read_line` realization remain separate dependencies.

Fixed-view length and indexed writes use the exact existing mutable field loan
at a machine or block parameter. Length means the current live extent, never
capacity. A write validates the typed index, byte, and saved length against that
extent before changing one byte; it cannot resize storage or alter owner length.
State transfers capture the original referent binding before fuel and commit,
reject aliasing mutable arguments, and preserve writes through calls and suspension.
Shared immutable backing detaches on mutation, preserving sibling values and
the destination's untouched suffix. External boundary replacement remains its
separate existing contract.

The settled [bounded-input contract](../../../../wiki/spec/resources/bounded_input.md)
does not use this owner-replacement behavior: it writes within a supplied slice
and returns a count plus line/EOF/full outcome, with ZII Invalid reserved for no
result. Migrate line-input callers, checked providers, and interpreter/native
consumers together. Existing replacement/forwarding tests pin their current
binding behavior, not new line-input acceptance. Ordinary whole-field stores
remain independently supported. Encoding qualification needs checked library
validation of the written prefix; buffer writeback itself grants none.

## Fixed byte arrays

`start_artifact_with_structural_arguments_and_byte_arrays` supplies explicit
initialized fixed-array contents by structural argument and relative field path.
Every supplied value must match a real unqualified `FixedArray(u8, N)` and contain
exactly `N` bytes. Duplicate referents, mistyped paths, and fabricated storage
reject; an opaque root does not imply initialization. `structural_byte_array`
observes the original backing by referent identity and path.

Ordinary Unit helpers may borrow the whole array or an admitted record-field
array as a mutable byte view. Calls and block transfers retain that binding;
length remains `N` and indexed writes preserve all other elements through fuel
suspension. A fieldless array binding is distinct from the bounded-field loan,
so the external `replace` callback cannot resize an array. This host-input route
does not implement source-owned array construction or native array/view storage.

## Boundary responses

[effect_results.rs](src/effect_results.rs) distinguishes Unit, scalar, and opaque
structural values. The current structural response has exact type,
qualifications, opaque identity, and an empty path. Linear results, result claims,
projected qualifications, and sum discriminator/payload inspection remain
unsupported. Preflight rejects unsupported result requirements before the host
effect; validate the response before committing result/claim custody.

Structural entry records and fixed arrays containing bounded integer fields
require explicit contents for every bounded field, including unread fields.
Startup validates each exact integer carrier and range before establishing
custody; missing, duplicate, or conflicting aliased fields reject. Unbounded
integer fields keep their existing deferred initialization check. Arrays are
visited incrementally, so missing contents do not require expanding their shape.
Opaque entry inputs and host results recursively containing bounded integer
fields still reject, as do bounded sum or mixed entry shapes without a selected
case discriminator. Their type identities do not supply complete field or
selected-payload values with which to validate the restrictions. Result preflight
rejects before the handler runs; this fence does not prohibit independently
valid selected scalar-case construction or supported exact internal transfers.

`EstablishScalarCase` materializes the declared selected payload from existing
typed scalar values, including independently proved restricted integers. The
interpreter carries its exact case and field identities through structural
return and internal-call suspension; case inspection stages the selected scalar
parameters before whole-result disposal. Empty payloads share this representation.
Affine case results support ordinary whole-root cleanup on returns and edges.
Opaque host values still cannot supply case observations. Passing an internally
constructed case as a structural call argument remains unsupported by the
interpreter's argument presentation; scalar and borrowed-view inputs with a
scalar-case result use the existing call path.

Host response validation is not allocation or freshness verification. The host
must return a legitimate owned value. Rejection preserves interpreter bookkeeping,
not effects the host already performed. Completion followed by budget suspension
must not repeat the effect. This carrier does not extend native provider support.

Residual disposal validates and charges the owning edge before mutation. On a
jump, scalar arguments materialize before disposal and successor binding. An
exhausted budget leaves residual ownership live. The exact producer/result root
and claim map survive suspension; no partially moved ancestor becomes whole again.
