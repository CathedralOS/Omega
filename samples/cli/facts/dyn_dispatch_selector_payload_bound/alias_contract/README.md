# Named public payload bound

This is another production of the [payload-bound fact](../README.md), under
`RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY`: a transparent checked mathematical
definition expresses the public requirement's existing guarantee.

Dispatching a trait method through a dynamic trait carrier whose contract
establishes `result <= 88u64` preserves the return bound across the selector and
data lanes, discharging exact subtraction `88u64 - result`.

The parent production's values, operation, inline implementation and transport
shapes are unchanged. Only the public guarantee is spelled
`within_payload_bound(result, 88u64)`, a public typed mathematical definition of
`value <= ceiling`. The original inline-contract production remains in the parent
folder. This is one fact with two contract productions.

At baseline `e06334c63dc4a811b0bcf9f530cbc91b14b23eac`, optimized producer
`b3a895c26149208c72bfa1f186e0c26bee9486ca59746de925d0f7fb5f8f5520`, the
local alias production refuses exact subtraction in `main` local `diff`.
The public predicate was lost by result-bound projection, and proper result
intervals were not retained on saved call observations.

The repaired optimized compiler executes
the root and all nine places with native exit 0 and Terminal exit 0 on
`linux_x86_64`. Run `omega run --both <project>/main.omg` for the root and each
of `local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`,
`other_state`, `other_machine`, and `target_body`. All retain four hosted
`ProgramEntry` bindings and expected exit 0.

Each place has an `invalid/main.omg` twin that changes only the public alias's
ceiling from 88 to 89. The implementation still returns 38 and still guarantees
88, but its private guarantee cannot replace the public requirement's weakened
guarantee. All nine twins reject with `exact arithmetic` at the consuming
subtraction. The fixtures are pinned as
`tests/omega/pass/facts/dyn_dispatch_payload_alias_through_<place>_exit` and
`tests/omega/fail/facts/dyn_dispatch_payload_alias_through_<place>_public_bound_weakened`.

Establishment comes from checked implementation refinement. Predicate projection
consumes the checked, reduced mathematical term with exact captured public
result and immutable formal identities. It never opens an implementation body
or supplies storage authority. The saved result is an invocation observation
carrying its public interval, including a proper interval; it is not an
invitation to replay an effectful dynamic call. Calls and returns preserve that
observation through the existing transport shapes. Unknown operators, mutable
formal values and unrepresentable bounds supply no interval.

The inherited selector/data separation, four target Build bindings, Terminal
lowering, ABI realization, indirect-call emission and authored runtime oracle
remain the parent fact's mechanism. The measured matching-host runs exercise
that full path; bindings alone establish no foreign-host run. Raw command,
producer, source, engine and rejection records are retained in the task's
ignored evidence checkpoint.

This production establishes integer alias result-bound transport and
public/private contract separation. The [IEEE alias production](../../dynamic_float_refinement/alias_contract/README.md)
establishes literal IEEE entailment separately. Suspending requirements,
crash/ranking support, package slot metadata, product parity and foreign-host
execution remain open in the parent task.
