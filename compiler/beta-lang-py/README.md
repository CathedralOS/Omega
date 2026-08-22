# `compiler/beta-lang-py/` — legacy mixed Beta reference tooling

This directory is historical migration state, not a Beta language rung and not
a standing architectural component. It groups tools by their current Python
implementation rather than by responsibility. The target ownership split is
defined in
[`repository_structure.md`](../../wiki/architecture/bootstrap_lattice/repository_structure.md).

## Useful reference and refinement tools

- `beta_interp.py` is an executable Beta reference used by compiler-correctness
  and input-exhaustion tests.
- `beta_symbolic.py` reconstructs source meaning for instruction-level
  refinement experiments.
- `symbolic_loop_check.py` and the fuzz generators test those reference and
  reconstruction paths.
- `beta_parser.py` owns the shared source lexer/parser and tuple-AST contract.
- `bc2.py` is an optional separately written compiler backend. Active meaning
  and symbolic tools no longer import compiler code merely to parse Beta.

These programs are untrusted. Their value is diagnostic: disagreement exposes a
bug or unsupported case. None grants an artifact authority.

## Why DDC is not an architectural requirement

The removed `diverse-double-compilation.sh` compared the Rust-cold-started Beta
compiler with `bc2.py` and was formerly described as closing a Thompson gap.
That ruling has been superseded by
[D5](../../wiki/architecture/bootstrap_lattice/decisions.md#d5--checked-refinement-not-ddc-closes-compiler-provenance).

The lattice does not trust compiler ancestry. It requires a lower-rooted check
that the exact produced artifact refines the canonical meaning of the exact
source. Once that check exists, a second compiler adds no soundness:

- two correct compilers may emit different artifacts;
- two incorrect compilers may agree;
- byte agreement establishes neither semantic correctness nor proof soundness;
- maintaining a second complete compiler creates an accidental second language
  specification.

The current `bc.beta` fixed point proves dependency closure but the complete
cold-start source-to-artifact refinement edge remains open. Removing the old
comparison does not widen that gap and closing the gap does not require another
compiler. The remaining reference comparisons are optional bug-finding tools,
not principal lattice gates.

## Migration

Retain tools by role rather than by host language:

| Content | Target owner |
| --- | --- |
| interpreter and semantic fuzzing | Beta reference meaning or Beta refinement tests |
| symbolic evaluator and loop reconstruction | `bootstrap/assurance/refinement/beta/` |
| reusable `beta_parser.py` source recognition | the narrowest reference/refinement owner that needs it |
| optional `bc2.py` compiler backend | retain only while it provides unique differential coverage |

The directory can disappear once those responsibilities have explicit owners.
