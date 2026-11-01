# Calling policies from retained Terminal declarations

`main` checks two independent boundary calls and exits 0. The portable inspection
executes the authored calling policy against `ReferenceEntry` using only the
retained/decoded Terminal artifact, after releasing the source session.

The first argument is a one-field record holding a reference to `PolicyReferent`.
Its pointee body is deliberately not demanded by the inspection. The checked
foreign transport role must still produce an 8-byte, 8-aligned reference field,
not reconstruct the source type or invent an owned storage declaration.

The inspection checks that this exact reference edge still has an identity-only
target, validates the field graph, then executes the policy to produce the real
argument/return placements. Existing rejection, fuel, target and nominal-identity
controls remain active. Run with `OMEGA_CORPUS_HOST_PROVISIONS=1`; an `unrequested`
runtime observation is not execution evidence.

The native callback fixture `pass/calls/callback_mixed_formal_entry` separately
checks the same canonical reference-geometry consumer on a real foreign call;
its retained/decoded controls also reject a substituted pointer/descriptor size.
