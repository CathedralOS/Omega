# Portable bootstrap checks

Copy this directory with the winning chain. Run the checks from any working directory with Python 3 and the executable under test. The fixtures, expected results and request framing travel together.

| Check | Command | Observations |
| --- | --- | --- |
| Gamma evaluator | `python3 check.py gamma /path/to/evaluator` | Framed source and stdin, exit status, output bytes, traps and malformed-source refusals. |
| Epsilon evaluator | `python3 check.py epsilon /path/to/evaluator /path/to/packed-evaluator.delta` | Exit, stdin, trap, refusal and malformed-envelope bytes. The request includes the digest of the supplied closure. |
| Epsilon input views | `python3 check.py epsilon --profile input-view /path/to/evaluator /path/to/packed-evaluator.delta` | Cursor snapshots, EOF, bounds, immutability and ordinary field lookup. |
| Omega program | `python3 check.py omega /path/to/compile-and-run` | The command receives the source path as its last argument and executes it. The program must exit 42 with empty stdout and stderr. |

A route supplies the adapter for the interface it implements. The compiled routes also use these vectors in their differential checks. The sources in `omega/` exercise the bootstrap compiler through its existing entries. The Epsilon evaluator entry binds its packed source digest. The portable Omega `program.omg` retains its published bytes. The executable-gate controls use comma-separated Omega transition arms.

Construction, seed audits, rebuild equality and route-specific limits live with the candidate. The portable checks exercise observable behavior.

The Epsilon grammar spells a view `&[u8]` and a receiver `&mut self`. The default Epsilon profile exercises that grammar. Change #4220 migrated Omega borrows to `@` and also changed two Epsilon examples. The published portable vectors copied those examples. [Change #4220](https://github.com/CathedralOS/Omega/pull/4220) · [Epsilon grammar](https://github.com/CathedralOS/Omega/blob/e488ba404bf/bootstrap/interpreted/4_epsilon/LANGUAGE.md#3-grammar)

Run `epsilon --profile historical-at` with the same evaluator and closure to reproduce the original five inputs and expectations. The compiled evaluator passes two and refuses the exit, stdin and trap programs at byte 167. Run `epsilon --profile syntax-controls` to check the two `@` refusals, longest operator matches, infix precedence and associativity, a missing operand, and valid and malformed escapes in both kinds of literal.

Quoted-byte controls check delimiter and high-byte selection, wildcard fallback,
and duplicate decimal, Boolean and escaped spellings. The evaluator passes all
five language observations and all fourteen syntax controls.
