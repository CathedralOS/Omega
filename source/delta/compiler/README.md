# Delta compiler work

The canonical Gamma-written Delta compiler and admitted tape remain absent. The
selected architecture requires that compiler to emit canonical Gamma source;
the promoted Gamma compiler then emits Beta, and Beta alone encodes Alpha.

Noncanonical implementation evidence is test-owned under
[`../../../tests/delta/`](../../../tests/delta/), not retained beside the
selected source spine. None of those experiments defines a selected edge or
amends [`../LANGUAGE.md`](../LANGUAGE.md). The canonical compiler still needs
algebraic data, exhaustive `match`, `Bytes`, complete checking, proper tail
calls, profiles, and exact resource outcomes before it can compile Epsilon.
