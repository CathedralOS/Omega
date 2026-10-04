# Conditional structural return cleanup

Prepare with `omega update --project tests/omega/pass/drops/conditional_structural_return_cleanup_exit`, then run `omega run --both tests/omega/pass/drops/conditional_structural_return_cleanup_exit/main.omg`.
Both engines must print exactly `BA` followed by a newline and exit 0.

Generic forwarding returns the initially unready A owner and cleans the ready B sibling. The caller repairs A before explicitly dropping it. The selected return transfers its result before executing the remaining owner's hook; the other branch must not run a cleanup. The source is unchanged from the original conditional cleanup customer.

The fixture's reviewed Console provider explicitly promises finite normal byte output. It does not strengthen the standard library's output contract. Build declarations cover the four hosted targets; execution was checked on macOS ARM64.

`python3 tools/corpus_gate.py --both --filter drops/conditional_structural_return_cleanup_exit` checks the exit and output oracles. Lowering unit controls also exercise both selections, direct and forwarded calls, changed custody, and missing or opposite branch prerequisites.
