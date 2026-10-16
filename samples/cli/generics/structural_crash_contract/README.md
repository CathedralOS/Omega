# Structural crash contract

`omega run --both samples/cli/generics/structural_crash_contract/main.omg`
must return the authored exit 70 in both engines after ordinary local package
review. A borrowed call retains a record-identity crash ceiling over a Boolean
and an exactly eight-element UTF-8-qualified array. The ceiling permits a crash;
the empty callee does not actually crash.

Baseline 6d0b8da07bd refuses Terminal production because the crash predicate
reader still classifies qualified fixed arrays as variable-fill byte carriers.
The matching existing corpus regression is
`tests/omega/pass/traits/runtime_qualified_byte_crash_equality_exit`.

The repaired pipeline retains a typed proof identity rather than expanding it
into executable field comparisons. Native and Terminal execution both return
70 on macOS ARM64. Corpus controls distinguish proof identity from IEEE or
authored executable equality, and reject stale operands after content writes
or descriptor replacement.
