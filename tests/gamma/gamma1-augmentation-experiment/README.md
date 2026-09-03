# Gamma1 augmentation experiment

This test-owned experiment inserts a source-only layer between authored Gamma
and the unchanged Gamma compiler:

```text
Gamma1 source -> Gamma0 lowerer -> Gamma compiler -> Beta -> Alpha
```

Gamma1 adds three column-zero, LF-terminated declaration forms:

```text
cell NAME
const NAME HEXWORD
text NAME ASCII-THROUGH-END-OF-LINE
```

`cell` allocates the next cell and defines `NAME` plus `set_NAME`. `const`
defines a named word that pushes the given literal. `text` defines a word that
emits the exact remaining ASCII bytes. Every other line is copied byte for byte,
so executable Gamma bodies retain current Gamma semantics and are not parsed by
the lowerer.

The lowerer is 193 lines / 6,254 bytes and compiles to a 6,259-byte Alpha tape.
Its implementation is intentionally plain Gamma0. The surface gate checks
interpreted/native agreement, an exact readable lowering receipt, compilation
through the selected Gamma and Beta path, and execution producing `frame`.

## Delta discriminator

The streaming scalar Delta compiler uses this layer directly. Its authored
source changes as follows:

```text
Gamma0 streaming compiler: 657 lines / 26,783 bytes, 19,757-byte tape
Gamma1 streaming compiler: 666 lines / 27,081 bytes
lowered Gamma0 compiler:    689 lines / 29,899 bytes, 22,762-byte tape
Gamma1 lowerer:             193 lines /  6,254 bytes,  6,259-byte tape
```

The Gamma1 compiler has 23 named-cell declarations, 8 named layout constants,
20 exact-text declarations, and zero authored `output-word` sites. Its generated
Delta receipts remain byte-for-byte identical to the Gamma0 experiment.

The local readability improvement is real: state names, table bases, row widths,
and emitted fixed tokens are visible without decoding addresses or ASCII words.
The cold-start total does not yet improve. Counting the lowerer, the current
authored surface is 859 lines. This layer pays only if reused by enough
larger trusted customers, or if its own reconstruction becomes substantially
smaller through disciplined self-augmentation.

Declared stack effects are deliberately absent. Unchecked declarations would
be comments with misleading authority; checking effects across user words,
branches, and recursion is not a tiny textual lowering. Existing `# ( -- )`
comments remain available where documentary effects help.

This is evidence for a possible internal Gamma stratum, not a proposal for a new
permanent rung or new runtime semantics.
