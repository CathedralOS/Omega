# L2 Resolver

Contract in progress. The first fixtures cover the lookup of an ordinary
machine declared in another file of the same package.

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

The next contract sections cover lexical scopes, file-local imports,
package visibility, builtin names by role, overloads and identities across
stages. [Resolution and visibility](../../wiki/spec/language/modules.md#resolution)
