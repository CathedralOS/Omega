# Copied case before a later argument

A copied Present payload of 7 stays readable after a later argument replaces its source with Absent.

This row advances **BUILD-OWNED-TARGET-SELECTION** and the existing **STATE-LOCAL-VALUE-FRONTIER** prerequisite found during **CML4**. Their broader acceptance remains open.

Every place preserves `read_payload(chosen, set_to_absent(&mut chosen))` on `Slot [copy]` (the field place uses `holder.chosen`). The first argument completes its independent copy of `Present { value: 7 }` before the second replaces the source with `Absent`. The caller establishes `read_payload`'s declared `before in Slot::Present` requirement for that exact completed copy. The generic reader proves its payload access from the declared premise.

The parameter, forwarding machine and selected implementation retain their original Present preconditions. The nested `inspect` retains its original signature without an added requirement: its inferred private obligation must hold on **every** closed arrival. Lexical nesting supplies no runtime capture or ambient parent premise. A second Absent arrival, an unknown entry operand or a replacement before the first argument must defeat that inference.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [Mutable local](local/main.omg) | [Absent input](invalid_local/main.omg) |
| Field | [Holder field and adjacent canaries](field/main.omg) | [Absent input](invalid_field/main.omg) |
| Parameter | [Mutable owned parameter](parameter/main.omg) | [Absent input](invalid_parameter/main.omg) |
| Call result | [Constructor-returning call](call_result/main.omg) | [Absent input](invalid_call_result/main.omg) |
| Match join | [Both arms and distinct routes](match_join/main.omg) | [Absent in both arms](invalid_match_join/main.omg) |
| Const name | [Whole case from INITIAL](const_name/main.omg) | [Absent constant](invalid_const_name/main.omg) |
| Another state | [Closed private arrival](other_state/main.omg) | [Absent arrival](invalid_other_state/main.omg) |
| Another machine | [Explicit wrapper contracts](other_machine/main.omg) | [Absent input](invalid_other_machine/main.omg) |
| Target body | [Selected ordinary implementation](target_body/main.omg) | [Absent input](invalid_target_body/main.omg) |

Each twin differs only in its initial `Present { value: 7 }` constructors becoming `Absent {}`; the match twin changes both arms. Build bytes, full public contracts, argument order, payload read, later write and runtime observations remain identical. The expected refusal names the immediate consumer: `read_payload`, `apply` or `forward`.

`observe_replacement` observes the Slot inside the activation that owns the replacement. It returns the original observed value only for Absent, otherwise sentinel 99, preserving the wrappers' unrestricted `u64` result and original reach/crash ceiling. Field canaries 101/107 detect neighboring writes. Match executes true and false with routes 17/23; each invocation finishes its checks before the next begins. The selected target also checks that its caller's independent Slot remains Present with payload 7 after the callee's owned Slot becomes Absent. These are post-operation runtime oracles, not added proof premises. Absent is a tag observation; it does not demonstrate clearing dead payload bytes or a particular physical copy strategy.

Additional private-arrival opposites preserve the core operation:

- [Second Absent arrival](private_arrivals/second-absent-arrival/main.omg).
- [Unknown root arrival](private_arrivals/unknown-root-arrival/main.omg).
- [Earlier overlapping replacement](private_arrivals/earlier-write/main.omg).

The nine pairs and three additional refusals are mirrored under `tests/omega/{pass,fail}/facts/`.

The implementation follows the existing owners:

- **Establishment:** construction establishes Present/payload 7; declared contracts or checked all-arrival substitution establish the reader's actual completed operand. A goal never establishes its own classifier.
- **Identity and lifetime:** detached copies retain their own case/payload. Exact prefix/storage facts account for the later write; mutation does not donate stale source facts.
- **Composition:** wrappers substitute actual operands and retain their contracts. The tag-only shortcut requires whole-body tag preservation; other cases use the existing full-prefix/private-arrival proof rather than bypassing it.
- **Soundness:** unknown, Absent and already-replaced arrivals refuse. Copy, affine and linear custody remain independent of whether a callee binding is mutable.
- **Checked-to-Terminal Psi:** children execute once in authored order. Owned aggregate ingress retains its ordinary transfer; the scalar current-value storage check applies to scalar parameters. Reference access, loan and claim joins remain unchanged.
- **Provider settlement, layout and ABI:** one ordinary `SelectedCopiedCaseApply` implements the exact requirement, selected explicitly for all four hosted targets. Entry bindings and sum representation remain ordinary Build/Terminal data.
- **Instruction selection and activation storage:** caller and callee own distinct copied Slots; the replacement and caller-copy checks expose incorrect shared homes or lost writeback.
- **Emission and execution:** the authored exit is zero in both engines on the observed matching host. Four-target image preparation establishes artifact formats, not execution on foreign hosts.

Original exact optimized 3c9 observations established eight positive Source/native/Terminal agreements and the wrong refusal of the nested positive. The first repair (2261ae) then made the unchanged nested positive check and execute 0/0 while rejecting second-Absent and unknown-root arrivals. Strengthened ordinary places subsequently passed on actual df5; the selected target exposed a separate owned-mutable aggregate custody refusal. [Hosted capture 41451849306](https://github.com/CathedralOS/Omega/actions/runs/41451849306) built the combined optimized CLI and runner at exact `1f3c4da0217`: all 21 Source cases have their intended outcomes (nine positives and twelve refusals), all nine pins and all nine public programs execute native 0 / Terminal 0 on Linux x86_64, both runtime counterfactuals trap, and all four target images are emitted. Only the owned 21 Source and nine Linux runtime golden blocks are updated; other blocks and headers remain unchanged. The three private-arrival opposites are part of those twelve refusals. Four-target artifacts do not claim execution outside the observed Linux host. Earlier weak match/write oracles and wrong-label/setup attempts remain historical evidence, not successful controls.
