# finite family returned descriptor

Owning outcome: **FINITE-GENERIC-DISPATCH**.

A finite-family method on a borrowed record returns a descriptor of the original backing, and reading its first byte yields 11.

Canonical contracts: [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary), [dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families), [returned views](../../../../wiki/spec/language/lifetimes.md#returned-views), and [target families](../../../../wiki/spec/language/machines.md#target-scoped-families).

At `036605466510d2361fd579cf9fd97b722e070beb`, the frozen borrowed-record source passes normal update, then `run --both` exits 200 with `argument referent requires a reference carrier`. The parameter/state composition also exposes `captured reference leaves have no resources`. A stored pointer variation reaches the same root-coordinate problem and then exposes native result transport that considered only moved carrier metadata.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Run each positive with `omega update --project <place> --target windows_x86_64` then `omega run --both <place>/main.omg`. All nine positives and reject twins are mirrored under `tests/omega/{pass,fail}/facts/finite_family_returned_descriptor/`. Every positive binds all four hosted targets and declares exit 0. The local resumption measures Windows x86-64; earlier Linux observations retain their original producer identity, and other hosts are unmeasured for this repair.

Requirement evidence:

- Establishment: source admission owns the incoming record and its complete stored-reference frontier. The provider returns that backing, not the address of its private carrier.
- Identity and lifetime: public and captured origins use executable structural-formal coordinates. Incoming content profiles retain every admitted leaf at the operand snapshot; known replacement captures and unknown writes remain authoritative. The reject changes only provider storage to an invocation-local backing and must diagnose that it does not outlive the call.
- Composition: the same record-backed descriptor survives local/field/parameter/call-result/join/const/nested/other-machine/target-body transport and forwarding through a returned record and state argument. The length branch supplies bounds for reading the runtime oracle; it cannot establish source custody. `const_name` first materializes the named array value into owned runtime storage, then borrows it.
- Soundness: no input-type relation creates a pointer or access permission. Synthetic resident-content profiles contain lifetime debt only; the actual outer view remains separate. Additional [pointer-field](pointer_field/main.omg) and [escape](pointer_field/invalid/main.omg) witnesses challenge stored-pointer crossings.
- Replacement composition: [replaced source](replaced_source/main.omg) installs an incoming shared pointer in borrowed caller storage before selecting and returning it, and checks that both outputs read 11. Its [twin](replaced_source/invalid/main.omg) changes only the installed backing to invocation-local storage and rejects with `does not outlive the call`. At fixed baseline `7df8b4d92070d83d357849fb2eb77e05c61f36a4`, normal `run --both` succeeds but the retained audit rejects the shared pointer's simultaneous stored/returned outputs as duplicate custody. The audit now forms independent shared receiving occurrences from the actual source contract and stored-effect summary; exclusive output transfer still rejects duplication.
- Checked-to-Terminal Psi: normal checked call/result custody and state transport feed canonical Terminal production and the Terminal interpreter; no alternate source execution route is introduced.
- Provider/layout/ABI: selector and data arguments remain separate, both finite tuples retain one common descriptor result, and native pointer metadata follows either moved storage or an existing typed resident observation. [Target qualification](target_qualification/main.omg) has every required body; its [invalid twin](target_qualification/invalid/main.omg) is valid source whose requested realization must reject a missing target body.
- Instruction/frame realization and native emission: real `run --both` observations check native and Terminal exit 0, including the first-byte payload. No ISA, frame, encoding, or permission rule is changed.

The resumption's ignored `build/slot9-resume-evidence` records frozen baseline inputs, immutable optimized producer digests, command exits, engine observations and exact-candidate review. Retained auditing stays explicit rather than becoming a phase of normal compilation. Passing transport alone does not establish automatic cleanup, production package approval or other-host native acceptance of the parent task.
