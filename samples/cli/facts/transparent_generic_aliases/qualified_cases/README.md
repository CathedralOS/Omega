# Alias-qualified cases

Fact: A transparent generic alias retains its underlying case identity and payload type through qualified construction and matching.

The imported `Chosen<T [copy]> = Choice<T>` constructs and matches Some42 and Empty17, with explicit `Choice<u64>` interchange. A qualified generic record pattern supplies42 too; it distinguishes record and case selectors without classifying them by path length. The sole twin changes only the Some payload from the carried u64 to true and rejects for the ordinary bool-to-u64 construction-field guard.

Linux x86-64 reference acceptance: [41401809299](https://github.com/CathedralOS/Omega/actions/runs/41401809299), actual source `6defc740839f0ca4b7d99d970e6d2d1f806a8837`. Both sample and corpus positive observations have native exit0 / Terminal exit0 / agreement; both negative observations match the unchanged authored diagnostic. CLI SHA256 `0475ef8fb6050da588346b66f711b6cb5a6c8a330bc7d208e8e61c403d16fc47`, corpus runner `dfa28e0eaaf293704fae96b9787d61811bf6361992b8dd799d2cc8ba96788007`, opt1. Preparation174.943s; checking8.798s, native/Terminal7.982s and sample pair21.408s.

The unchanged existing `runtime_tuple_case_destructure_exit` control passes checking and native70 / Terminal70 / agreement in [41408004845](https://github.com/CathedralOS/Omega/actions/runs/41408004845), source `7899446a0dc71e930a7b70f8c5abaeb890575227`. CLI `b9ecf2f072cb090bbf5264c84f19c2ef1cd82b372156a77cbf2a894beb564fa4`, runner `fdb537d7fe0b7554c59dbee6a01738342750dc9b7ecec33126d443bf04322732`, opt1; preparation440.564s, checking59.515s, native/Terminal58.553s. It covers tuple payload aliases, Named targets and two computed subjects evaluated once. Earlier false reports retain its inherited postfix failure and the subsequent missing computed-case coverage.

No alias replay or observation rekey is required for these control repairs: the frozen alias programs never apply a second invocation suffix to a completed call, and their single-subject case tails keep the existing exact-case coverage path. Computed tuple rows extend the shared finite-domain coverage algorithm without changing execution or treating unknown operations as proof. The test-only Debug snapshot update has no production effect. The actual main integration preserves main's broader runtime type-owner rule plus the new expression owner and updates the moved namespace imports; required CI checks the integrated source.

Historical macOS source `363cb9e6a2200663ea536e700352df48167908a9` directly refused these final programs at the old qualified-pattern path ceiling. Ordinary positive package review separately stopped at historical Std boundary admission; no native acceptance is claimed for that baseline. Later failed current-source probes remain false, including lost selector/value distinction, generated membership purpose, deferred capture scope, descriptor projection before local retirement and equality-only exit coverage.

The field, parameter and call-result places pass their first scoped observation in [41411074164](https://github.com/CathedralOS/Omega/actions/runs/41411074164), source `417ca5344d798a60a7a938e1fea7164a5690cbe3`. All three sample/corpus positives have native0 / Terminal0 / agreement, and all three boolean-payload twins match the original field-type rejection. CLI `79e116ee78d197e7eae72c77d3ab6eda973a952d058461c48d7ec61dde72b8dc`, runner `ad6b45c1f6c3c0feb388975d0132f7cea64bae2d9141c4eec6db8c410e0396e0`, opt1. One producer: preparation1130.067s, authentication1.839s, checking23.393s, native/Terminal20.763s, six sample observations58.926s. No compiler changes or earlier-place reruns were needed.

| Place | Actual source | Native / Terminal | Sole twin |
| --- | --- | --- | --- |
| Local (root) | `6defc740839f` | 0 / 0, agrees | Intended bool-to-u64 rejection |
| [Field](field/README.md) | `417ca5344d79` | 0 / 0, agrees | Intended bool-to-u64 rejection |
| [Parameter](parameter/README.md) | `417ca5344d79` | 0 / 0, agrees | Intended bool-to-u64 rejection |
| [Call result](call_result/README.md) | `417ca5344d79` | 0 / 0, agrees | Intended bool-to-u64 rejection |

This establishes four of nine places. Match join, const name, nested machine, other machine and selected-target transports remain open, together with other hosts, Product, expanded-only alias application inference, bare alias-case expression matches and remaining binder/pattern compositions. Raw reports and exact measured-source bundles are retained outside scratch at `/Users/zcanann/Omega-build/alias-qualified-cases-20261101/` and `/Users/zcanann/Omega-build/alias-qualified-case-transports-20261102/`.
