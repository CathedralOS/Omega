# Proof-kernel gates

These shell entry points compile and exercise the checker implementations,
replay the corpus, and enforce soundness, cross-check, and operational-seam
policy. Gates consume implementations, tools, and corpus data through their
canonical owner paths; they do not implement the derivation judgment.

Run the principal suite from any working directory with:

```sh
sh bootstrap/assurance/proof-kernel/gates/test.sh
```
