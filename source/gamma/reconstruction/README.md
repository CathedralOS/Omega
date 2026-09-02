# Gamma evaluator reconstruction

`gamma_evaluator_reconstructor.gamma` is a diagnostic Gamma program that reads
the canonical addressed Beta evaluator source and emits its exact Alpha tape.
It implements the mnemonic, operand, address-assertion, comment, separator, and
`db` forms used by that source. It does not embed the evaluator tape.

This is a reconstruction triangle, not a compiler fixed point and not another
trusted bootstrap edge. The Beta compiler and Gamma reconstructor independently
produce the same tape from readable source; the Beta path remains authoritative.
The executable equality gate lives at
[`../../../tests/gamma/evaluator-reconstruction.sh`](../../../tests/gamma/evaluator-reconstruction.sh).