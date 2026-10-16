# Component assumption recording

The build declares one exact component-assumption digest through the public
`Build::accept_component_assumption(builder, digest)` call. The customer exits
70 through the standard ProcessExit provider. Accepting a component assumption
does not accept the project's package permissions, confer host authority, or
replace normal package review.

Run `omega update --project samples/cli/independent/component_assumption_recording
--target linux_x86_64`, inspect and decide the exact project review, then resume
with `omega update --resume --project samples/cli/independent/component_assumption_recording`.
Run `omega run --both samples/cli/independent/component_assumption_recording/main.omg`.
Native and Terminal must both report exit 70 with no application output.

On Windows x86-64, use `windows_x86_64` for the update target, then run the
same `omega run --both` command without `--target` (an explicit run target is
compile-only). This path previously stopped at
`PostAllocationMachine(MachineEffects(Analysis(CatalogSelectedKeysMismatch)))`:
the Windows allocator omitted four port/model-specific register declarations
already present in the ISA effect catalog. With those catalogs consistent,
native and Terminal both exit 70; the authored source and admission stay unchanged.

The checked digest is `0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef`.
`validation/observe.rs` independently reads the exact row from complete source
checking, checking its bytes, count, selected Build and authored span. This
observation uses the repository's explicit fixture policy, separately from the
normal project's accepted lock. Its runtime mode uses normal package preparation
and captures the exact Terminal artifact retained by native production.

The component mode inspects a genuine one-port-write fixture without executing
its privileged operation. Its independent policy fixes service 1, port 1016 and
value 65. A Python SHA-256 oracle independently reconstructs the assumption
identity. The verifier accepts the unchanged component only with that exact
assumption and rejects absent and changed digests. The observer also supplies each consumer’s exact recorded Build policy to the
verifier: authored acceptance succeeds, while missing and changed authored
digests reject. This verifies assumption admission; it does not claim deployment
of an independent provider edge. Source controls reject malformed
hex, short digest, wrong arity, swapped operands and constructed Build values;
explicit reborrow and uppercase spelling work, and a same-named user declaration
grants nothing. The package entry's reserved `builder` parameter convention is
unchanged.

Link the observer against one existing optimized compiler build:

```
python3 samples/cli/independent/component_assumption_recording/validation/build_observer.py TARGET_DIRECTORY OBSERVER
python3 samples/cli/independent/component_assumption_recording/validation/check.py --compiler OMEGA --observer OBSERVER --output EVIDENCE_DIRECTORY
OBSERVER runtime ABSOLUTE_MAIN_OMG EVIDENCE_DIRECTORY
```

These validators never change package policy or regenerate expected results.
