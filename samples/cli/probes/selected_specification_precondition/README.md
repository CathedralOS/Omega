# Selected specification preconditions

Run `omega run --both samples/cli/proofs/selected_specification_precondition/main.omg`
after normal package preparation. Native and captured Terminal execution must
both exit 70. `oracle.json` states the independently authored identity/result
row and execution oracle.

The theorem repeats the exact saturation premise required to form
`selected_identity<Modulus>(value)`. Its statement citation is erased only after
that selected premise and the identity conclusion have been checked. The sample
originally rejected the theorem in both generic and specialized bodies, although
the corresponding unselected structural call passed.

The fixture family `selected_specification_requires_*` keeps missing-premise,
wrong-machine, wrong-subject and false-successor controls separate from the
positive substitutions. Cauchy remains a downstream regression, not this repair's
oracle. The compiler must substitute exact callable and value binders together;
neither matching spellings nor erasure provides missing evidence.
