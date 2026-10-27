# Private collection formation

Owner: `CHECKED-PACKAGE-REUSE` in [TASKS.md](../../../../../TASKS.md). This is a std package-review prerequisite in the nested-machine invocation family.

Every private arrival carries an in-bounds index, so the helper can read bytes[index].

The exact baseline `85e1ce7073884a9d1ea83aeef055030d0f44daff` refuses the guarded private helper because collection formation consults only its own declared entry facts. The shared ordered-prefix owner must also account for every closed private arrival. No helper gains a handwritten duplicate precondition, and no lexical parent contract is assumed.

- [local](local/main.omg), [reject twin](local/invalid/main.omg)
- [field](field/main.omg), [reject twin](field/invalid/main.omg)
- [parameter](parameter/main.omg), [reject twin](parameter/invalid/main.omg)
- [call_result](call_result/main.omg), [reject twin](call_result/invalid/main.omg)
- [match_join](match_join/main.omg), [reject twin](match_join/invalid/main.omg)
- [const_name](const_name/main.omg), [reject twin](const_name/invalid/main.omg)
- [other_state](other_state/main.omg), [reject twin](other_state/invalid/main.omg)
- [other_machine](other_machine/main.omg), [reject twin](other_machine/invalid/main.omg)
- [target_body](target_body/main.omg), [reject twin](target_body/invalid/main.omg)

The same bound and read operation occur in every place. Each twin weakens only the strict bound to a non-strict bound. Distinct first/last array values, tags, and an out-of-bounds fallback make wrong reads or wrong branches observable. All four hosted entries are authored. Copies and rejection/exit oracles are pinned under `tests/omega/{pass,fail}/facts/nested_machine_private_index_*`.

Establishment/composition: dominating guards and exact completed actuals supply the index-to-length relation. Every private incoming occurrence must be covered; an escaping callable or unsupported cycle cannot provide it. Identity/lifetime: shared slices retain their collection identity and captured scalar indices; mutable storage controls remain separate obligations. Soundness: missing, weak, or incompletely covered premises reject. Psi/Terminal and Omega/native checking, argument placement, array layout, instruction encoding and emitted image execution require fresh host observations before this row is closed; the other three hosts remain unverified here. Parent checked-package persistence, native product persistence, and timing acceptance remain open.

Host evidence: optimized precursor `a7c52e0d539484edbb9c27861aea3bf4c0047296eb7fae22f1649e245779fca8` (fixed base plus recorded successor-completion patch) ran all nine positives with `omega run --both --target macos_arm64`, exit 0, and all nine twins refused for their fact-specific diagnostic. The Terminal/native agreement includes the authored wrong-result traps. Raw source hashes, commands, elapsed times and streams are retained in `build/checked-reuse-20261027-85e1/successor-completion-rows.json`. These are host observations of that precursor; other targets and the parent persistence/timing requirements remain open.
