# Dynamic IEEE guarantee refinement

Inline exact IEEE value `0.25` guarantee refines public inclusive f64 interval
`[0, 0.5]`; the dynamic call observes `0.25`.

The unchanged root selects the named implementation `Quarter` as `dyn Fraction`,
calls the public `Fraction::get` slot and crashes unless its completed result is
`0.25`. The implementation separately proves its exact result guarantee.
The checker must prove that guarantee implies the public lower and upper bounds.
Neither an implementation body nor a selector's known target grants a caller a
stronger public contract.

At base `97fca7a18f744b0f7c0b54811b7d41b586d6bfb7`, the optimized compiler
rejected both public bounds before execution:

```text
machine `Quarter::get` realizes `Fraction::get` but does not guarantee its `ensures result >= 0.0`
machine `Quarter::get` realizes `Fraction::get` but does not guarantee its `ensures result <= 0.5`
```

Run `omega run --both samples/cli/facts/dynamic_float_refinement/main.omg`.
All positive build roots bind the four hosted ProgramEntry targets; only a
matching execution host establishes that target's native result.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | `local/main.omg` | `local/controls/outside_interval/main.omg` |
| field | `field/main.omg` | `field/controls/outside_interval/main.omg` |
| parameter | `parameter/main.omg` | `parameter/controls/outside_interval/main.omg` |
| call result | `call_result/main.omg` | `call_result/controls/outside_interval/main.omg` |
| match join | `match_join/main.omg` | `match_join/controls/outside_interval/main.omg` |
| const name | `const_name/main.omg` | `const_name/controls/outside_interval/main.omg` |
| another state | `other_state/main.omg` | `other_state/controls/outside_interval/main.omg` |
| another machine | `other_machine/main.omg` | `other_machine/controls/outside_interval/main.omg` |
| another target body | `target_body/main.omg` | `target_body/controls/outside_interval/main.omg` |

Each twin changes only Quarter's own guarantee and returned value to `0.75`.
Its body still satisfies its own guarantee; the unchanged public upper bound
must reject at implementation refinement. The match witness exercises both
selector-producing arms. The const witness stores the selector itself, preserving
its exact implementation and closed trait application, rather than naming a
separate numeric constant.

The implication judgment compares authenticated subjects under their callable
binders and selected IEEE operator/carrier meaning. It weakens positive ordered
bounds and equality using the existing IEEE predicate laws. Negated order and
`!=` supply no non-NaN bound. Endpoint strictness, f32 landing and signed zero
retain their own IEEE meaning; integer complement laws do not apply. Different
subjects, formats and public/private declaration identities cannot donate facts.
Ordinary flow writes retain their existing invalidation rules.

Private constant selectors retain a checked symbolic implementation selection;
they contain no user instance, address or cleanup obligation. The same constant
eligibility and exact trait-application checks apply even to unused initializers.
Public constants still owe canonical identity and cannot bypass that requirement.
Runtime lowering uses the existing selector table relocation and CallDynamic ABI.
Proof-prefix traversal captures selector construction as fresh opaque code identity;
its static implementation name is not a runtime data operand. Subsequent calls and
writes retain their ordinary effects and invalidate observations as before.
Anonymous float arguments land once in the selected explicit formal's format.

Primitive comparison spellings must select the canonical source requirement
through ordinary operand-driven resolution, then retain its exact occurrence into
Terminal Psi and the selected provider plan. A raw IEEE operation without that
selection is insufficient for native admission. Named helpers and foreign domains
retain ordinary source exposure; loaded lookalikes cannot obtain sealed meaning.
An authored `Float::equal` lookalike rejects earlier as a duplicate normalized
provider schema; that control proves refusal before admission, while the legal
foreign-domain scope witness challenges operand resolution itself.

Authority: [conformance contracts](../../../../wiki/spec/language/conformances.md),
[machine substitution](../../../../wiki/spec/language/machines.md#substitution),
[IEEE numeric meaning](../../../../wiki/spec/language/numeric_values.md),
[constant eligibility](../../../../wiki/spec/language/constants.md), and
[implementation selection](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md).

This row discharges literal IEEE-bound implication and copyable selector constant
transport under DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION. The parent retains
broader implication, suspending requirements, owned result/claim routes, crash
refinement, dispatch ranking, package review and product-source parity.

Acceptance requires native exit 0 and Terminal exit 0 for the root and all nine
places, and the public upper-bound rejection for every twin. Qualification
receipts retain the actual producer, revision, target and observed outcomes;
this row does not close the parent capability.
