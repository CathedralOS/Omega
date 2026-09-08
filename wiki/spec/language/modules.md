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
Public interfaces cannot select private declarations. An authorized public
conformance may retain private implementation identities without granting
consumers direct selection of them.

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
