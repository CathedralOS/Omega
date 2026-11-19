# L2 Resolver

Contract in progress. The first implemented operation, `resolve_machine_calls`,
binds direct calls to ordinary machines in one admitted package. Run its
fixture suite with `cargo test -p omega-resolver`.

## Inputs and outputs

For these fixtures, L2 receives parsed files, their package identity and
their logical paths relative to the package root. For each call, the expected
result identifies the selected machine or the unresolved name at its source
span. Tests should supply files in both orders and expect the same result.
The V2 build model requires declaration identities to survive independent
compilations. [Stage identity](../../PLAN.md#v2s-build-model-a-chain-of-stages)

## Directory namespaces and sibling lookup

A file contributes declarations to the namespace named by its parent
directory. Files in that directory see each other's declarations without a
`use`. A subdirectory introduces a distinct namespace. The filename itself
contributes no segment. [Directory namespaces and import scope](../../wiki/spec/language/modules.md#directory-namespaces)

Both fixtures use one package, `game`, with no imports. Each contains the
same two files of Omega source. Paths below are relative to the fixture root.

| Fixture | Declaring file | Expected result for `score()` in `combat/main.omg` |
| --- | --- | --- |
| [Same directory](fixtures/accept/sibling_machine/combat/main.omg) | `combat/scoring.omg` | Select package `game`, machine `combat::score`. |
| [Child directory](fixtures/reject/child_machine/combat/main.omg) | `combat/ai/scoring.omg` | Report unresolved `score` in the caller, line 2, byte columns 5 through 9. |

## Implementation boundary

The caller supplies the settled files of a package with canonical,
slash-separated logical paths. Source discovery and path admission belong
upstream. The resolver collects top-level declarations before binding calls.
The output identifies each call by logical file and byte span and its target
by package, directory and name. Renaming a file within its directory preserves
the target identity. Tests also cover forward and recursive calls, nested calls
in arguments and same-spelled machines in separate namespaces.

This operation binds call names only. It accepts ordinary machine parameters
and simple bodies. When a parameter shadows the callee, a file introduces an
import or local binding, or a call needs qualified selection, it reports
`Unsupported`. Competing same-named machine declarations at a call produce
`CompetingMachines`. These outcomes identify work for later L2 rules. Within
the supported subset, a missing callee produces `Unresolved` with the precise
name span. [Resolution and visibility](../../wiki/spec/language/modules.md#resolution)

The next contract sections cover lexical scopes, file-local imports,
package visibility, builtin names by role, overloads and identities across
stages. [Resolution and visibility](../../wiki/spec/language/modules.md#resolution)
