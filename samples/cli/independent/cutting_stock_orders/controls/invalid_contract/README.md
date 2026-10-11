The complete standalone source has no provider or external dependency. `cut`
requires a positive length and sufficient remaining stock under the documented
machine requirement rule (`wiki/spec/language/state_contracts.md`). `control`
sets remaining stock to 4, then requests a cut of 5. The check must reject this
call requirement. `expected.txt` freezes the diagnostic fragment before execution.
Any parsing, arithmetic-body, entry, package or custody failure is a wrong-reason
rejection and does not pass this control.
