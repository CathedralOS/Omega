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

## Intrinsic lookup in the Gamma interpreter

The interpreter tests the head of every list while checking and executing Gamma. The original table stored two words per intrinsic. Each probe unpacked a class and a length, then called the string comparator. The revised table stores one word per spelling and uses its position for the class. This removes 128 bytes from the evaluator tape, taking it from 8,575 to 8,447 bytes. The interpreter packs short tokens once per visit. A head longer than eight bytes goes straight to ordinary function lookup.

The 153,051-byte Delta compiler at `0a0b233a152` checked the three inputs below with the same 4,301-byte support bundle. Both evaluator tapes ran on the Linux x86-64 Alpha seed. Instruction counts cover the contiguous Beta routines from `token_equals` up to `is_open`, including their uses outside list-head dispatch. The counter builds incremented a register before each instruction in that interval and appended its little-endian value at halt. Removing those eight bytes yielded the ordinary output in every case. CPU figures are medians of eight uninstrumented runs each, alternating the order of the two evaluators.

| Delta request | Previous instructions | Revised instructions | Previous / revised CPU seconds | Exact result |
| --- | ---: | ---: | ---: | --- |
| Concatenate the input with itself | 13,580,205 | 5,386,853 | 0.545 / 0.478 | 3,082-byte Gamma receipt |
| Return an unbound name | 8,122,735 | 3,209,424 | 0.393 / 0.361 | Status 1, DCOUT space 1, code 14, byte 33 |
| Flip the last support byte | 6,335,092 | 2,531,634 | 0.324 / 0.301 | Status 1, DCOUT space 5, code 2, byte 3,055 |

The three sources use `(def main ((source Bytes)) Bytes BODY)` followed by LF. Their bodies are `(bytes_concat source source)`, `missing`, and `source`. For the last request, XOR the final byte of the packed support with 1. Frame each with Delta profile 1 and supply it as sealed input to the Gamma-written Delta compiler, assembled from `delta_compiler.gamma` followed by its manifest members. The copy request visited 80,263 list heads and called the generic string comparator 614,356 times before and 33,014 after. [Request and compiler packing](../../../tests/bootstrap/compiled.py)

The earlier 151,983-byte Delta compiler from the interpreted matrix also ran on the revised evaluator. Given the original 99,270-byte linked Delta program, it returned the same 81,742-byte Gamma receipt, SHA-256 `cbd56e337c1d0c2374ba36fdfff71f1cebe5a2cc1b8d912a5a68d6b0843718bf`. The new run used 87.06 CPU seconds. The original measurement recorded wall time only. [Interpreted matrix](interpreted-gamma-matrix.md)
