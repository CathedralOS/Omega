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
This whole plain-owned route does not admit partial/qualified owners or mutable
borrowed block parameters.

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
not make bounded storage interchangeable with views at ordinary calls.
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
opaque identity without that binding supplies no mutable loan. Immutable-view
operations still require immutable bindings. Native byte-field forwarding and
`read_line` realization remain separate dependencies.

The settled [bounded-input contract](../../../../wiki/spec/resources/bounded_input.md)
does not use this owner-replacement behavior: it writes within a supplied slice
and returns a count plus line/EOF/full outcome, with ZII Invalid reserved for no
result. Migrate line-input callers, checked providers, and interpreter/native
consumers together. Existing replacement/forwarding tests pin their current
binding behavior, not new line-input acceptance. Ordinary whole-field stores
remain independently supported. Encoding qualification needs checked library
validation of the written prefix; buffer writeback itself grants none.

## Boundary responses

[effect_results.rs](src/effect_results.rs) distinguishes Unit, scalar, and opaque
structural values. The current structural response has exact type,
qualifications, opaque identity, and an empty path. Linear results, result claims,
projected qualifications, and sum discriminator/payload inspection remain
unsupported. Preflight rejects unsupported result requirements before the host
effect; validate the response before committing result/claim custody.

Host response validation is not allocation or freshness verification. The host
must return a legitimate owned value. Rejection preserves interpreter bookkeeping,
not effects the host already performed. Completion followed by budget suspension
must not repeat the effect. This carrier does not extend native provider support.

Residual disposal validates and charges the owning edge before mutation. On a
jump, scalar arguments materialize before disposal and successor binding. An
exhausted budget leaves residual ownership live. The exact producer/result root
and claim map survive suspension; no partially moved ancestor becomes whole again.
