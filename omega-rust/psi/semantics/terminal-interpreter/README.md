# Terminal interpreter

Start at [lib.rs](src/lib.rs). Execution follows verified
[calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
and [structural ownership](../../../../wiki/spec/terminal-psi/ownership.md).

## Cyclic execution

Ordinary interpretation accepts verified natural-ranked scalar/immutable-view
graphs under the [control contract](../../../../wiki/spec/terminal-psi/control_flow.md).
The legacy one-machine structural Unit countdown uses its own verifier entrance:
it reconstructs `0 < remaining` as the unsigned `1 <= remaining` premise and
checks exact subtraction before creating resumable state. This interpreter
acceptance grants no native, fixed-fuel, provider-installation, or mixed-work
authority.

## Calls and work

The interpreter charges a call before entering its owned frame. Exhaustion inside
the callee resumes without paying or invoking that call again. A callee crash
retains its original site and edge charge, with no fabricated caller crash.
Ordinary recursive call graphs remain fenced until their tail/ranking evidence
is admitted; cyclic block execution has its separate verified route above.
Fixed-work composition distinguishes normal return bounds from crash bounds:
only a normal return composes the caller's remaining work.

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
