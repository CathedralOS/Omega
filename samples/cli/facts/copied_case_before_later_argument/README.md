# Copied case before a later argument

A copied Present payload of 7 stays readable after a later argument replaces its source with Absent.

Parent: **STATE-LOCAL-VALUE-FRONTIER**. This is a prerequisite found during **CML4**; neither parent is closed by this row.

Every place preserves `read_payload(chosen, set_to_absent(&mut chosen))` on `Slot [copy]`. The first argument finishes its copy of `Present { value: 7 }` before the second replaces the source with `Absent`. The caller proves `read_payload`'s declared `before in Slot::Present` requirement for that completed copy. The generic callee proves its payload read from this declared premise; it cannot obtain hidden scalar or case assumptions from particular callers.

The parameter, forwarding-machine and four hosted `apply` bodies declare the corresponding `chosen in Slot::Present` premise. The internal named-state place retains its ordinary predecessor join; it is not an ordinary-call entry specialization. No extra branch, proof guard, operation, argument reordering or data-value change is introduced.

Each invalid twin changes only the initial `Present { value: 7 }` to `Absent`; both match arms change together. The declared case requirements, payload reader, later write and final value7 comparison stay identical. Before execution, each invalid expects the declared-requires diagnostic for its immediate consumer: `read_payload` in local/field/call-result/match/constant/internal-state places, `apply` in parameter/target bodies, and `forward` in the forwarding-machine place. Actual observations must confirm these authored expectations; a mismatch must be preserved and diagnosed.

| Place | Transport |
| --- | --- |
| [local](local/main.omg) | Mutable local. |
| [field](field/main.omg) | Owned `Holder.chosen` field. |
| [parameter](parameter/main.omg) | Mutable owned parameter with declared case premise. |
| [call result](call_result/main.omg) | Constructor-returning call. |
| [match join](match_join/main.omg) | Same initial case in both arms. |
| [const name](const_name/main.omg) | Whole case supplied by `INITIAL`. |
| [another state](other_state/main.omg) | Internal successor-state join. |
| [another machine](other_machine/main.omg) | Two generic wrappers with declared premises. |
| [target body](target_body/main.omg) | Four hosted bodies with the same declared premise. |

- **Establishment:** source construction supplies Present/payload7; the caller establishes the declared case requirement at the exact completed argument.
- **Identity and lifetime:** copied facts describe the completed value, while views keep their own live-storage obligations. The later source write cannot retag the detached copy.
- **Composition:** all nine transports retain the same copy/write/read operation; generic wrappers propagate explicit requirements and internal state edges retain ordinary joins.
- **Soundness:** a generic body is checked from its declared premises. Caller-specific constants/tags cannot become hidden callee assumptions. Missing capture evidence remains unknown for caller obligations.
- **Psi lowering:** each child evaluates once, with the first copy completing before the later write.
- **Settlement and layout:** ordinary sum layout and argument order carry the copied selected payload; all four hosted entry bindings remain authored.
- **Native realization:** actual observed host results and Terminal inspections must substantiate tag/payload preservation; bound targets alone do not claim execution on other hosts.
- **Emission:** the corrected positives retain the authored zero-exit oracle. Fresh accepted preparation, native/Terminal products and exact invalid diagnostics remain required. Earlier C80/C3/8df observations used an uncontracted generic reader and are historical observations of that mistaken premise, not validation of this corrected row.

Source/build mirrors and authored exit oracles remain under `tests/omega/{pass,fail}/facts/copied_case_before_later_argument_through_*`. The invalid diagnostic files now name these exact requires-call obligations. They are frozen pre-execution expectations, not results inferred after a run. This row covers declared-precondition caller snapshots; it does not by itself demonstrate a new implementation gap or justify publishing caller tags into generic bodies.
