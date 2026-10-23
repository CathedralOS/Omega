# Consuming recoverable replacement through a retained alias

Fact: a consuming recoverable result restores its exact borrowed place and preserves disjoint storage.

Parent: **BORROWED-STORAGE-RESTORATION** in [TASKS.md](../../../../TASKS.md).
This row exercises ordinary result sums while an ownership window is open,
including a second extraction after repair through a retained exclusive alias.
Failure returns the removed token; success disposes that token and constructs
its replacement. Neither outcome cancels the restoration debt.

`repair` first consumes `holder.token` through `alias`, returns the result through
`Outcome`, and repairs through `alias`. It then extracts again through that alias
and repairs through the original owner path. Both calls may mutate only the
disjoint `sibling`. Running both failure and success leaves token value 44 and
sibling value 19. The caller checks those actual storage values and traps on
any other result; zero exit is the authored oracle.

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Their
`invalid/main.omg` twins replace only the last establishing store with disposal
of the returned token. They must reject because the original hole remains open,
with `` `holder.token` must hold a value again `` identifying the exact restoration obligation.

The field variant uses a nonzero-offset envelope and checks its disjoint prefix.
The parameter and state variants transport the repaired owner to an observer;
the call-result and match-join variants return it through completed producers.
The target-body variant checks the same repair body for all four hosted targets.

## Requirements

1. **Establishment:** the linear token has an ordinary constructor; the completed
   outcome producer and case consumer supply owned replacement custody.
2. **Identity and lifetime:** the exclusive alias retains its captured storage
   across referent stores. Each extraction creates a separate owned value while
   debt remains at the original caller field; the alias descriptor is retained.
3. **Composition:** success and failure return the live obligation through a sum;
   the same captured place is restored twice, through alias and owner spellings.
   The nine transports preserve the operation, bound and caller-visible result.
4. **Soundness:** the missing-repair twins cannot use returned ownership as proof
   that absent storage is established. Loan exclusion, exact type, linear
   disposal and caller debt remain independent obligations.
5. **Psi 05–07:** extraction and establishing store retain exact opening/result
   coordinates into Terminal Psi. The caller's home is used directly; no staged
   caller copy or rollback is introduced.
6. **Omega 00–02:** ordinary Build entry bindings and structural call/return plans
   carry Token, Outcome and Holder with their existing field/tag geometry.
7. **Omega 03–06:** native calls preserve the alias and sibling home while the
   detached token/result homes have independent lifetimes.
8. **Omega 07–09 and emission:** the native executable's storage observations
   must agree with actual Terminal interpretation through `omega run --both`.

The canonical contracts are [source ownership windows](../../../../wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
and [Terminal restoration](../../../../wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).
Recoverable failure is an ordinary result sum under [effects](../../../../wiki/spec/language/effects.md).
This row does not establish physical suspension/resume/cancellation or completion
on unexercised native hosts. The parent capability remains open.

## Baseline and validation

The reconstructed local witness was frozen before compiler edits with SHA-256
`d7094bcb7e35b6091fd11c5a56c79271dab2fea90bdcb81de5257eec1cdc0030`.
At main `d6cf554fd1cbf20551983d29bae26f5cf3b81664`, a freshly built optimized
compiler wrongly rejected its second extraction: `alias must hold a value again`
(statement 3), although the final store repairs its original `holder.token`.
The original saved worktree was absent in the resumed environment; this is a
fresh reproduction of the recorded semantic witness, not reuse of its old logs.

Validation records live in the worktree's `build/restoration-evidence/`.

All nine positives and the nine intended rejection twins were exercised on the
saved cloud Linux x86-64 host. Every positive observed native exit 0 and Terminal
exit 0. The sibling-counter composition fixture observes token 4311 and sibling 11 (decimal accumulation distinguishes call order);
the descriptor-rebinding fixture observes original token 42, other token 7 and
sibling 19. Its wrong-place control rejects specifically for `holder.other`.

The implementation removes descriptor reassignment from referent-store route
replay, separates referent value custody from descriptor custody, admits the
completed producer's actual value type, adopts extraction results with ordinary
call preparation, and keys the lowering debt on the captured reference origin.
Physical accesses retain their checked routes and authority; source checking
and Terminal reconstruction still enforce exact place, type and custody.
