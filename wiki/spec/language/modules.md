# Modules, imports, and visibility

[Build declarations](../build/declarations.md) define the package directory,
role, source lineage, and dependency graph. A source file belongs to its package
by location; it does not repeat a package declaration. Module names organize
declarations within that boundary and participate in name/artifact identity.

## Names and imports

```text
name_path   := IDENT ("::" IDENT)*
module_decl := "module" name_path ";"
import_decl := "use" name_path ";"
```

`::` selects a static name; `.` selects a field or callable through a value.
Imports select logical names, never filesystem paths, and do not execute code.
External selection requires a requester-local direct dependency and the target
declaration's visibility. Fully qualified spelling and inferred receivers obey
the same rule. Tooling may discover an enclosing package; source cannot walk
parent directories to gain undeclared reach.

The [package boundary](../packages/boundaries.md) distinguishes authored
selection from carrying a foreign nominal type through a declared dependency's
API. Carrying does not add a direct dependency or grant selection authority.

### Import scope and exposure

Imports affect the importing source file, not sibling files that declare the
same logical module. This is the ordinary import rule; it does not restrict
visibility of the logical module's own declarations. Import exposure is not
transitive: another module's imports do not become this file's imports.

| Authored selection | Foreign domains exposed for carrier-qualified lookup |
| --- | --- |
| Import a declaring module | Its directly declared public domains. |
| Import one domain declaration | That exact domain. |
| Import one ordinary machine | No sibling domains. |
| Import a carrier type | No foreign domains merely because they attach to it. |
| Load a dependency or another source file | None merely because it was loaded. |

A module import does not recursively expose domains from descendant modules or
modules it imports. Repeated selection of one exact declaration adds no competing
candidate. Dependencies resolve their bodies under their own source imports;
consumer imports cannot reinterpret already selected declarations.

### Foreign attached declaration paths

An exact foreign-domain address begins with its declaring package/module, not
the carrier owner's namespace:

```omega
use ui_policy::screen::Point::OnScreen;
```

Here `ui_policy::screen` declares `Point::OnScreen`. The attachment is resolved
in that declaration's context to the exact carrier, for example geometry's
`Point`; a caller's same-spelled type cannot redirect it. The domain and carrier
retain independent owners. A module import such as `use ui_policy::screen;`
exposes the module's directly declared public domains instead of just this one.
Short carrier-qualified selection must match both the resolved carrier and an
exposed domain. Direct qualified selection names only the exact declaration and
does not expose siblings. All selections retain ordinary dependency and visibility
requirements; a declaration path is not a grant to select a private carrier.

Importing a domain permits name selection, not membership, minting authority,
access to private representation, or an implicit change of operators. A value
may carry an exact foreign qualification and its evidence through an API without
importing that domain for authored lookup. Carrying neither erases that evidence
nor exposes other declarations of the domain's package.

Current support is documented beside
[source resolution](../../../omega-rust/psi/pipeline/README.md#resolution-and-closed-instance-normalization).

## Resolution

Name resolution considers, in order:

1. Local bindings.
2. State parameters.
3. Machine parameters.
4. Receiver fields through the explicit `self` receiver.
5. Imported names.
6. Fully qualified package/module paths within the declared dependency set.

Ambiguous imported declarations reject. Compiler traversal order cannot select
between them. An explicit receiver field projection is not an implicit bare-name
alias for that field.

Distinct visible declarations competing for the same carrier-qualified domain,
case, or machine name reject; neither carrier ownership nor import order selects
a winner. Module growth can therefore introduce a collision in a broad import.
Narrow imports avoid exposure to unrelated additions. Diagnostics retain both
declaration owners, the exact carrier, and the imports exposing the conflict.

## Visibility

Independently nameable data, domains, traits, machines, boundary requirements,
wire schemas, operators, constants, and complete named conformances are private
unless marked `pub`. Carrier qualification does not inherit visibility.
Fields, cases, states, and trait requirements instead inherit their exact
semantic owner's visibility. An attached declaration head independently selects
its carrier and therefore needs authority to name that carrier.

Ranking measures are private proof machinery; `pub measure` rejects. A public
termination guarantee does not publish its implementation's ranking witness.
Requirement-local `as Name` labels have no standalone package visibility.
Public interfaces cannot select private declarations as consumer-nameable API.
An authorized public conformance may retain private implementation identities
without granting consumers direct selection of them. A public domain's
[issuer catalog](../resources/authority.md#private-issuer-routes) may likewise
retain private trait requirements or exact machines accessible to its author.
That authorization metadata grants no consumer call, conformance, or private-type
selection permission. Other public signatures and predicates retain their
ordinary exposure rules; this is not general public access to private names.

There is no `export` item relabeling dependency-owned declarations. `export` is
not reserved. Ordinary public wrappers expose behavior under the wrapper owner's
API. Public lookalikes cannot create compiler intrinsics or acquire their
availability. Publishing proof vocabulary/contracts does not establish every
application or accept the assumptions of a bodyless claim.

## Public structural data

Publishing structural data publishes its field names and shape. Authorized
consumers may read, construct, and update values subject to field types,
invariants, borrowing, and qualification requirements. Visibility is not
confidentiality, unforgeable authority, or an ABI policy.

Constructing geometry or handle bits does not construct its routed authority or
provider provenance. Confidential state remains in custody outside the
observer's access. Changes to public shape change source/API identity;
independent binary compatibility and replacement follow explicit
[component contracts](../build/component_publication.md).
