# Required generic provider instances

Owning outcome: **PROVIDER-SELECTION-AFTER-TERMINAL** in `TASKS.md`.

Selecting the named `Identity` implementation must preserve every checked closed
`Forward::identity` obligation independently of which candidate bodies are emitted.
The ordinary entry calls Count=4 and Count=8; an uncalled ordinary machine records
Count=12. All three have the same runtime carrier shape.

`main.omg` traps if either invoked identity returns the wrong value. The source
oracle in `expected_provider_instance_arguments.txt` identifies the three actual
Const tuples and their invocation disposition. Existing corpus inspection captures
their checked requirement/body and receiving coordinates before releasing source,
then compares those obligations with retained and decoded Terminal catalogs.

The public Artifact refusal control removes only the uncalled Count=12 candidate,
leaving its body, the other candidates and the independent obligation in place.
The modified artifact crosses normal decoding, then target provider selection must
reject specifically for missing canonical instance coverage. No malformed checked
source or compiler-internal custody constructor supplies this control.

Native and Terminal execution both exit 0. Build binds all four hosted targets;
local observations qualify only the host actually executed.
