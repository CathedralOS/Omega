# Interpreted construction path

In this route the Gamma interpreter executes the Delta **compiler**. That
compiler translates the Epsilon evaluator from Delta to Gamma. The same Gamma
interpreter then runs the emitted evaluator on Epsilon source.

```text
alpha/                 audited VM
beta/                  self-reproducing compiler tape
gamma/interpreter/     Beta-written Gamma interpreter
delta/compiler/        Gamma-written Delta compiler
epsilon/interpreter/   Delta-written Epsilon evaluator
omega/compiler/        Epsilon-written bootstrap compiler
```

The construction target is:

```text
Alpha -> Beta -> Gamma -> Delta -> Epsilon -> bootstrap compiler
      -> Omega compiler -> reproducibly self-rebuilt Omega compiler
```

The seed includes the audited [Alpha implementation](../../../bootstrap/alpha/README.md) and
separately admitted [1,773-byte Beta tape](../../../bootstrap/beta/AUDIT.md). Reconstructing that
tape from source establishes identity; it does not remove the tape as a trust
input. Source and disassembly annotations retain historical paths so relocation
preserves their bytes.

The executable upper edge constructs exercised Omega programs. Full construction
of the product compiler and its self-rebuild remain unfinished. The product is
[the Omega product sources](../../../source), rather than another implementation
owned by this variant. Its implementation subset should keep bootstrap demands
small without changing the semantics of supported Omega facilities.

Cross-rung invocation resolves paths through
[`tools/bootstrap/paths.sh`](../../../tools/bootstrap/paths.sh). Existing identity,
source-closure and executable gates remain under `tests/bootstrap/interpreted/` and the
language-specific test directories. Reused artifacts and development accelerators
must remain distinguished from fresh reference-chain execution.

[The compiled VM peer](compiled-alpha.md) explicitly reuses these retained
sources. That reuse grants this architecture no preferred or selected status.
[Optional proof research](../../../tests/bootstrap/proofs/README.md) does not gate construction.

## Earlier Delta compiler measurements

```text
3,350-line / 147,840-byte canonical entry plus shared Gamma implementation
7-line / 195-byte nullary-ADT Delta fixture
  -> 3-line / 165-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
7-line / 186-byte payload-ADT Delta fixture
  -> 3-line / 230-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
7-line / 187-byte recursive-ADT Delta fixture
  -> 3-line / 425-byte Gamma receipt
  -> selected Gamma evaluation produces byte 3
8-line / 221-byte two-field recursive List fixture
  -> 3-line / 502-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
24-line / 767-byte three-field recursive rope fixture
  -> 7-line / 1,404-byte Gamma receipt
  -> indexing produces byte 0x42; indexing empty traps
7-line / 277-byte proper-tail List fixture
  -> 4-line / 568-byte Gamma receipt
  -> constructs and traverses 100,000 nodes through if, let, and match
10-line / 379-byte typed Bytes fixture
  -> 10-line / 1,184-byte Gamma receipt
  -> all five builtins produce byte 0x42
5-line / 209-byte skewed Bytes fixture
  -> 9-line / 1,000-byte Gamma receipt
  -> 100,000-node lookup produces byte 0x5a in bounded call context
11-line / 397-byte forward/mutual nominal fixture
  -> 3-line / 956-byte byte-identical Gamma receipt
  -> all nullary, unary, and three-field constructor shapes produce byte 7
946-line / 35,826-byte current Epsilon declaration prefix plus scalar entry
  -> exact 21-byte scalar Gamma receipt within the evaluator watchdog
12,018-line / 612,210-byte current Epsilon source plus checking entry
  -> measured 708,456-byte Gamma receipt
  -> checking gate requires 70 exact judgments
3,001-function / 66,266-byte scale fixture
  -> 78,271-byte Gamma receipt
  -> selected Gamma evaluation produces byte 199
  -> transforms within the staged gate's unchanged 30-second watchdog
```
