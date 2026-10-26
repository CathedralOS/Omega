# Target-family invocation lifetime

**PSI-TARGET-FAMILIES:** Every target-family member retains a fresh invocation lifetime and the returned item reference independently of captured context storage.

The selected callback returns the item while the context holds a reference to different storage. Each place has a twin that changes only one member's callback result to the enclosing lifetime:

- [local](local/main.omg) / [reject](local/invalid/main.omg)
- [field](field/main.omg) / [reject](field/invalid/main.omg)
- [parameter](parameter/main.omg) / [reject](parameter/invalid/main.omg)
- [call result](call_result/main.omg) / [reject](call_result/invalid/main.omg)
- [match join](match_join/main.omg) / [reject](match_join/invalid/main.omg)
- [const name](const_name/main.omg) / [reject](const_name/invalid/main.omg)
- [nested machine](other_state/main.omg) / [reject](other_state/invalid/main.omg)
- [another machine](other_machine/main.omg) / [reject](other_machine/invalid/main.omg)
- [target body](target_body/main.omg) / [reject](target_body/invalid/main.omg)

The [two-lifetime control](two_lifetimes/main.omg) separates the captured context lifetime from the item lifetime and uses them in the reverse order from their telescope. Its [return-relation twin](two_lifetimes/invalid/main.omg) changes the callback result to the captured lifetime; its [telescope twin](two_lifetimes/invalid_telescope/main.omg) adds a lifetime binder to one member. Both reject.

The callback must refine its invocation-lifetime family for every fresh region, including its returned input-source relation ([generics](../../../../wiki/spec/language/generics.md#invocation-lifetime-families)). All target members must be checked and retained ([configuration](../../../../wiki/spec/build/configuration.md#multi-target-compilation)).

At fixed baseline `f7681adbc9b1190fbc19abf217b28e62ea2fb037`, an optimized Windows producer rejects the positive with three static lifetime tuple custody diagnostics. The untargeted equivalent checks successfully. The twin additionally rejects for a different returned-view input-source relation. These are checking observations, not execution evidence.

- **Establishment and composition:** the selected callback returns its item; its declared result relation is retained through the family and each transport place. The runtime comparison observes which storage was returned and does not establish that relation for checking.
- **Identity and lifetime:** the captured context and callback invocation remain distinct; each twin changes one member's callback result relation and rejects. The tuple's declaration-local parameter follows its family member while the argument remains in the caller's lexical scope.
- **Soundness:** the repair transfers existing lifetime operands only across members of one retained family. It neither invents a region nor relaxes the universal callable requirement or borrow checks.
- **Checked-to-Terminal Psi:** each Linux `omega run --both` observation executes the checked call and returned reference in the Terminal interpreter.
- **Provider, layout and ABI:** each project binds `ProgramEntry` for all four hosted targets. Linux and Windows execution exercise selection, reference transport and calling plans; the arm64 targets remain unverified here.
- **Instruction, frame and emission:** Linux and Windows native execution reaches exit 0 and agrees with Terminal for each positive. The two-lifetime control also runs in both engines on Windows. These observations do not establish arm64 realization or the broader family, callback and provider acceptance.

The fixed-baseline and candidate run records are retained with the session. The full parent task remains open, including distinct target realizations from one Terminal artifact and Build companion, remaining portable consumers, open const scopes, evidence telescopes, callback conjunctions, and the named corpus controls.
