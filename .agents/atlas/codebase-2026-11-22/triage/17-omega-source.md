# Omega-written compiler findings

| Finding | Disposition | Evidence |
| --- | --- | --- |
| [Area 17](../digests/17-omega-source.md), section 5 and spot check 7. An invalid span can have the same spelling as a valid run of NUL bytes. | Fixed in the source-set comparison used by name resolution. Both spelling operations check the unit, its retained byte range and the half-open span before reading its length or bytes. | With the product source from `1ab250720c8c`, the source-span fixture hit its failure trap (exit 203). After the repair the same fixture reported `Psi exit 0`. It exercises a real NUL, an empty span at EOF, a missing unit, a reversed range and a range past EOF. Replay with `python source/subsystems/compiler/source_to_psi/tests/source_spans.py <omega executable>`. |
| Section 5, loan last uses (unit 088). A full 128-slot table forgot the next distinct local. | The ownership checker now refuses that machine with `CheckCapacityExceeded` at the unrecorded use. Repeated uses of a retained local still update its last statement at capacity. | The [last-use fixture](../../../../source/subsystems/compiler/source_to_psi/tests/loan_last_uses.py) runs the product scan with generated name expressions. Both engines accepted the old silent-drop control and rejected the assertion that the 129th use had been diagnosed. The repaired scan passes the diagnostic and retained-row checks on both engines. |
| A full proof-goal stack can drop work and still claim proof. | Already fixed on main in [#5178](https://github.com/CathedralOS/Omega/pull/5178). | The checker records overflow and refuses the proof. The kept fixture is `source/subsystems/compiler/source_to_psi/tests/fixtures/facts/run/goal_stack/main.omg`. |

Other area 17 findings remain under review.
