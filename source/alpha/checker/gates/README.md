# Checker gates

| File | Role | Deletion condition |
| --- | --- | --- |
| `test.sh` | Compact positive/negative discriminator coverage for the retained calculus. | Replace when the calculus changes; delete a case when another case distinguishes the same boundary. |
| `soundness.sh` | Focused invalid-certificate attacks. | Delete a case when the compact suite or a formal soundness check subsumes it. |
| `check-ref-diamond.sh` | The single complete independent checker cross-check. | Delete when replaced by a stronger independent formal check. |
| `semantics-diamond.sh` | The single bounded definitional-versus-operational equality seam. | Delete when the seam is formally proved, ceases to be canonical, or is no longer consumed. |

These gates do not implement the derivation judgment and do not form lattice
rows. Run them from any working directory.
