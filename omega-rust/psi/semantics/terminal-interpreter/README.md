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
