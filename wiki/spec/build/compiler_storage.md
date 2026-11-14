# Compiler storage

A build keeps its compiler working state in memory and leaves none of it behind
for a later build to recover. The filesystem holds only what someone
deliberately keeps: acquired sources, `Omega.lock`, pending review transactions,
requested outputs and explicitly published artifacts. So an old compiler-cache
directory, whether stale, damaged or unwritable, cannot affect a build, and no
build needs a cache cleared before it behaves predictably.

## The rule

Named decision: `no-implicit-compiler-storage`, ratified 2026-11-14. It settles
the owner question `native-cache-producer-authorization` by retiring the native
component cache rather than deciding who may write to it.

Compiler memoization and intermediate reuse are in memory and scoped to one
complete build invocation, including its dependency, Build and target work. A
long-running compiler process or daemon does not extend that scope: each
invocation starts with fresh compiler working state, and previous compiler-cache
contents are neither consulted nor used to recover intermediate results.
Explicit inputs, published artifacts and recovery transactions keep their own
lifecycles, listed below.

A build neither creates nor consults an implicit disk-backed compiler cache,
whether per-user, per-project or local to the invocation. This covers compiled
native components, package review results, metadata derived from sources, Build
execution results and proof-search results. Within one invocation, identical
compilation requests still share their work under the exact-input and
execution-context [reuse rules](scoped_execution.md#execution-contexts-and-reuse).
Requests that differ in a relevant input or context, such as Build versus
Product purpose, target, configuration or provider selection, are not merged
merely because they select the same dependency. This rule does not choose
between whole-package and per-machine compilation.

A temporary file needs a concrete implementation requirement. It lives in
scratch space the invocation owns, apart from requested outputs, and is removed
on normal completion and on handled failure. Anything left after a forced
termination is safe to delete and is never read by a later build.

## What is unchanged

| Kept on disk | Governed by |
| --- | --- |
| Acquired dependency sources and their offline copies | [Source selection](../packages/sources.md) and [resolver security](../packages/resolver_security.md) |
| `Omega.lock` and lock acceptance, including skipping package review when the lock matches | [Acceptance](../packages/acceptance.md#ordinary-compilation) |
| Pending review and publication transactions, which are recovered after an interruption and never deleted as disposable state | [Review transaction](../packages/acceptance.md#review-transaction) |
| Requested outputs and explicitly published products | [Component publication](component_publication.md) |
| How bundled and fetched libraries are distributed and selected | [Toolchain](../packages/toolchain.md) and [package sources](../packages/sources.md) |

An explicitly published artifact is an intentional output and a later declared
input, not hidden state found by path. A later build consumes it only under
[receiving-host admission](../terminal-psi/product.md#publication-and-separate-consumption);
its byte identity never authorizes its producer. Removing the cache closes one
way in, not that boundary.

## Why

Persistent compiler state carries its own ownership, cleanup, invalidation and
recovery problems. A stale or damaged entry can change a build its user believes
is clean, and the user may not know the entry exists. Authorizing the cache's
writers would make it trustworthy but not wanted. The cost of this rule is that
work repeats across builds; measure that cost rather than reintroduce implicit
storage.
