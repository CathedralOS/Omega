# Selected provider issuance

Only an admitted selected InitialSupply call establishes fresh Count1 ownership, preserved with exact backing through submit/complete and release.

This row establishes the selected-invocation introduction needed by **EXTENT-LIBRARY-TRANSFORMS / BOUNDARY-ISSUANCE** in [TASKS.md](../../../../TASKS.md). The admitted `InitialSupply::acquire` result supplies one owned buffer after successful fresh allocation. Writer submit/complete preserve its Count1 content and exact pointer; release consumes that buffer. Allocation, matching fields, quantity equality, and an ordinary call to the implementation supply no fresh authority.

The [root](main.omg) and [existing customer](../../../../tests/omega/pass/core/content_retained_custody_round_trip/main.omg) carry the same body, contracts, Count1 bound, and exit-zero oracle. Their ordinary entries own Main storage. Nested machines declare their own contracts; the failed allocation path has no normal result. All positive projects bind all four hosted targets and provide explicit allocation/release leaves for them.

| Place | Positive | Fact-sensitive reject twin |
| --- | --- | --- |
| local | [main.omg](local/main.omg) | [ordinary supply](local/invalid/main.omg) |
| field | [main.omg](field/main.omg) | [ordinary supply](field/invalid/main.omg) |
| parameter | [main.omg](parameter/main.omg) | [ordinary supply](parameter/invalid/main.omg) |
| call result | [main.omg](call_result/main.omg) | [ordinary supply](call_result/invalid/main.omg) |
| match join | [main.omg](match_join/main.omg) | [ordinary supply](match_join/invalid/main.omg) |
| const name | [main.omg](const_name/main.omg) | [ordinary supply](const_name/invalid/main.omg) |
| nested machine | [main.omg](other_state/main.omg) | [ordinary supply](other_state/invalid/main.omg) |
| another machine | [main.omg](other_machine/main.omg) | [ordinary supply](other_machine/invalid/main.omg) |
| target body | [main.omg](target_body/main.omg) | [ordinary supply](target_body/invalid/main.omg) |

Each twin changes only the selected supply call into `InitialSupplyProvider::acquire`, plus its diagnostic header. The match twin changes both supply alternatives. It retains the same provider bodies, quantity, backing contracts, and consumers. Pins under `tests/omega/pass/facts/selected_provider_issuance_through_<place>_exit/` and `tests/omega/fail/facts/selected_provider_issuance_through_<place>_ordinary_call/` copy these sources and Build declarations exactly.

## Requirements and evidence

1. **Establishment.** [External roots and issuance](../../../../wiki/spec/resources/authority.md#external-roots-and-issuance) authorize the exact selected admitted boundary. The raw allocation and ordinary implementation result remain unqualified. Twins require the missing `Buffer::Owned` obligation itself to reject.
2. **Identity and lifetime.** [Content custody](../../../../wiki/spec/resources/content_custody.md) retains exact invocation, quantity and backing correspondence. Both Writer contracts preserve pointer equality and their content equations. Runtime compares the restored pointer with the saved original before releasing it.
3. **Composition.** The same ownership survives each transport into `round_trip`; the field place moves the complete buffer out of an envelope, the match place retains both supply alternatives, and nested/ordinary/target helpers publish their own result obligations. No caller equality guard establishes ownership.
4. **Soundness boundaries.** Explicit Build acceptance and provider selection are separate from invocation. Source bodies prove conservation over existing accounts; the authorized supply boundary owns introduction. Ordinary invocation, nonzero pointers and constructor shape never substitute for that authority.
5. **Checked to Terminal Psi.** Checked content equations are discharged before runtime while pointer/custody inputs remain. The same program executes through Terminal Psi. Canonical reload checks encoding and representation foundations; it is not a claim of complete proof or lifetime verification. Crash-only callees introduce no normal result, cleanup or restored-loan receipt.
6. **Provider, layout and ABI.** Build records only actually executed, source-issued product requests through its original mutable receiver; product resolution occurs after graph admission. Provider settlement retains the selected invocation and its claims. Native realization transports the pointer-sized record through its declared calling plan; supplemental stopped-call controls cover scalar and indirect aggregate ABIs.
7. **Registers and activation storage.** Ordinary calls retain live inputs, caller storage and bounded activation funding. A call without normal completion still retains its declared ABI and argument homes. Supplemental controls read a borrowed record before exiting and preserve an observable constructor prefix and the normal match alternative.
8. **Encoding and native image.** The host observations require direct native execution and Terminal agreement, including exact exit/output controls, and canonical artifact reload. Ordinary call templates retain exact callee relocations and source-edge attribution through image publication. Four hosted bindings are source coverage; actual execution evidence is scoped to the recorded host.

## Current frontier

At optimized producer `5076dc7de0c73e6c2ace12741e09125d82752ab9`, root, customer and all nine places exit 0 in native and Terminal execution on macos_arm64 and reload through the canonical representation reader. All nine ordinary-call twins reject the intended Owned obligation. The same producer passes seven stopped-call/normal-alternative controls and two packaged-constant positives; the distinct nominal-carrier control rejects for its intended type mismatch. Authored output controls match both engines. The immutable qualification record retains exact sources, package locks, producer digests, raw streams and prior failed attempts. This closes this selected Count1 issuance row; it does not close its parent capability.

The governing contracts also include [Build declarations](../../../../wiki/spec/build/declarations.md), [scoped execution](../../../../wiki/spec/build/scoped_execution.md), [effects](../../../../wiki/spec/language/effects.md), [calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md), and [calling plans](../../../../wiki/spec/build/calling_plans.md). The optimized fixed-base producer `64894af1cfb73ab990eacaa45f24c68a4dccf790` refused the original valid Build selection before product resolution; raw baselines, exact producer digests and subsequent attempts remain in the issuance evidence ledger.

This row does not close physical Extent split/recombine, cross-provider shared backing, stale/replayed or duplicate supply admission, arbitrary geometry substitution, or Cathedral retirement/readback. It does not establish same-spelled nominal shadowing or execution on unavailable hosts.
