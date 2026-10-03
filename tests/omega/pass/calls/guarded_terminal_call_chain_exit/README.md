# Ordered terminal machine calls

Each selected arm invokes `mark` exactly once and forwards its result. Three
invocations select the first arm, second arm and fallback; the caller observes
20, 30, 40 and a mutation count of 3, then exits 0.

From the repository root, prepare the package-free project and run both engines:

```sh
omega update --project tests/omega/pass/calls/guarded_terminal_call_chain_exit --target macos_arm64
omega run --both tests/omega/pass/calls/guarded_terminal_call_chain_exit/main.omg
```

Native execution and the checked-tree interpreter both exit 0 on macOS ARM64.
The lowering regression also serializes, independently verifies, and executes
the Terminal Psi artifact without source. Mixed internal-state, value-return,
and machine-call arms share the same ordered decisions; forged call placement,
target, coordinate, access, and service evidence reject.

Captured calls retain their authored callable symbol. Signature and effect
joins resolve a machine head to its entry without replacing that source
identity. The ordinary call builder and its independent replay remain the
owners of argument, borrow, result, and contract checks.
