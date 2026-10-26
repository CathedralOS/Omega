# Target-family contract operands

**PSI-TARGET-FAMILIES:** Target-family member preconditions retain their operand identities, and a call with 42 satisfies the joined contract.

Each place calls the family with 42, satisfying all member preconditions. The invalid twin tightens the Windows member's bound from 512 to 41 and must reject the unchanged call. The family must check every member and retain their contracts independently of target realization ([configuration](../../../../wiki/spec/build/configuration.md#multi-target-compilation)); copied operands must retain their exact declaration identity.

- [local](local/main.omg) / [reject](local/invalid/main.omg)
- [field](field/main.omg) / [reject](field/invalid/main.omg)
- [parameter](parameter/main.omg) / [reject](parameter/invalid/main.omg)
- [call result](call_result/main.omg) / [reject](call_result/invalid/main.omg)
- [match join](match_join/main.omg) / [reject](match_join/invalid/main.omg)
- [const name](const_name/main.omg) / [reject](const_name/invalid/main.omg)
- [nested machine](other_state/main.omg) / [reject](other_state/invalid/main.omg)
- [another machine](other_machine/main.omg) / [reject](other_machine/invalid/main.omg)
- [target body](target_body/main.omg) / [reject](target_body/invalid/main.omg)

On fixed base `f7681adbc9b1190fbc19abf217b28e62ea2fb037`, all nine positives reject while forming copied member `<=` terms. Their twins additionally reject the false `42 <= 41` obligation. An optimized current-main `178897a3f89a7158387d7df5d4c14ee42e772701` producer reproduces the local positive and twin. The earlier malformed nested-machine draft is excluded from this evidence.

The scoped place-type query resolves a bare name by its retained symbol in the actual state telescope, so a copied contract does not need its original display spelling to match the receiving parameter. A resolved foreign or ambiguous symbol supplies no type. This transfer does not establish the caller's precondition or grant access authority.

- **Establishment and composition:** each call supplies 42, proving the conjunction of the four member preconditions; the twin makes one conjunct false without changing the call.
- **Identity and lifetime:** copied member operands resolve through their retained symbol in the receiving telescope; no display spelling or unrelated declaration supplies their type.
- **Soundness:** the query supplies a declared type only, not a value fact, an access permission or a proof of the precondition. Inconsistent or duplicate scoped symbol identities supply no type.
- **Checked-to-Terminal Psi:** the nine positives have executed in both engines on Linux and Windows; the corrected Linux nested-machine place was run separately with its explicit crash route.
- **Provider, layout and ABI:** each project binds all four hosted targets. Linux and Windows execution exercise their selected family bodies and calling plans; arm64 hosts are not yet established for this row.
- **Instruction, frame and emission:** native results agree with Terminal on Linux and Windows for all nine positives. Arm64 execution, pinned corpus, exact-candidate qualification and the remaining parent-task acceptance are still open.
