This witness advances `STATIC-MACHINE-MIGRATION`'s inferred structural callable
types under [generics](../../../../../wiki/spec/language/generics.md#inferred-callable-parameter-types).

The exact typed use in `origin` supplies the callback's input, result,
precondition, postcondition and termination promise. `relay` uses that origin
twice, and `forward` carries the same complete type through another binder.
The independent generic path forwards its type argument through a lexical
child. The executable oracle traps unless these paths return 37 and 43.
All four hosted entries are authored; qualification records identify the host
actually run.

The selected callback accepts input 37 and guarantees result 37. Its refinement
witness has the requirement's entry condition, calls the selected published
contract, and proves the required result relation at normal return. The
selected precondition is an independent call obligation; the selected body is
not a premise of this contract comparison. The proof-only result cannot publish
an executable closure; abstract contracts retain their outstanding body proofs.
Ordinary body, type, lifetime,
effect and crash checks remain required.

The [reject twin](../../../fail/generics/inferred_structural_callback_contract_rejected/main.omg)
changes only the selected callback's guarantee and body to result 38. It must
refuse that guarantee. The existing acquired-claim conditional-callback pair
under `concurrency` preserves the suspension and exact acquisition provenance
oracles while its forwarding binder leaves the callable type inferred.

Distinct structural constraints still need conjunction support. Open compound,
value or applied selections and captured enclosing lifetime correspondence
remain implementation work. This milestone does not close the parent task or
claim a complete fact row.
