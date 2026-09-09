# Package declaration and carried-type boundaries

The package is the dependency-reach boundary. `pub` exposes declarations owned
by the package; `build.omg` declares its dependencies and selections. Undeclared
aliases are not nameable, and fully qualified spelling does not bypass that
boundary. There is no export item that relabels a dependency declaration.
A subsystem needing a different dependency-reach set is a separate package,
not a hidden nested manifest.

[Modules and visibility](../language/modules.md) owns source path syntax,
resolution order, and public structural data access.

## Selection versus carrying a type

A direct dependency authorizes authored selection of its declarations. It is
not required merely because that package's nominal type arrives through an
already-declared dependency's API. Such a value may be moved, borrowed, stored,
returned, passed back through that API, and checked for multiplicity.

Carrying it does not grant access to its owner's methods, fields, cases,
operators, conformances, or ordinary consuming machines. Compiler-planned layout
and automatic cleanup are carried type semantics. The reserved owner-attached
`T::drop` hook is compiler-only; an authored `omega::core::drop(value)` is an
ordinary consuming call whose resulting cleanup plan remains carried semantics.
Borrowed erased views never acquire cleanup ownership of their referents.

[Reflection](../language/reflection.md) elaborates member selection under the
lexical author's dependency and visibility authority, not the generic caller's.
An owner may supply a checked visitor or conformance to another package. That
package invokes the supplied operation; carrying its type or metadata does not
authorize independently enumerating the owner's fields.

Exact foreign-domain qualification and evidence can likewise travel through an
API without importing the domain for authored lookup. Carrying does not expose
its declaring package's other extensions or erase its separate ownership.
Explicit domain selection follows [file-local import exposure](../language/modules.md#import-scope-and-exposure).

The transitive closure retains the exact type owner. Exact semantic dependencies
affect rebuild/content identity for private uses and also API compatibility for
public-signature uses. A coarse whole-package dependency may conservatively
over-reject until exact declaration edges exist; it cannot expand source
nameability. Owned erased payloads carry their exact movement/cleanup descriptor,
not a new declaration selection or trait-evidence grant.

## Exposure and declaration selection

Public contracts, data/domain facts, and trait contracts use public-interface
exposure. Executable bodies, internal states, local annotations/casts, owned
storage, and ranking witnesses remain private implementation. The public
termination promise is `terminates`, not the measure used to prove it.
A public interface cannot expose a private declaration for consumer source
selection. Private issuer references in a domain's authorization catalog are the
limited evidence-only exception described below.

Independently nameable declarations own their own visibility, including
carrier-qualified declarations. Only genuine members of one exact semantic owner
inherit visibility. An attached declaration head selects its carrier independently;
qualification does not admit a transitive-only owner. Exported boundary supply
has interface exposure even without ordinary `pub`.

Trait parents and body requirements normalize to semantic composition edges but
retain every authored selection. Generic bounds select their exact right-hand
trait; a qualified conformance selects both carrier and conformance. Subject
parameters, evidence binders, and local value roots remain lexical bindings.
Explicit and inferred conformance selections both need dependency authority;
inference is not a route to a transitive-only declaration.

An exact `satisfies` edge selects its trait and overload-resolved requirement,
or its exact operator/top-level boundary requirement. It follows the realizing
machine's interface exposure, including exported boundary/accepted supply.
Selection settles before supply admission; rejection of the supply does not
erase the declaration it selected. Nested machine-parameter contracts and
generic applications retain the same selections.

Each domain establishment route selects an exact trait requirement or concrete
machine, retaining its target kind and author-side dependency/visibility checks.
Repeated authored routes retain their occurrences even when equal semantic
alternatives normalize to one route. A public domain may authorize a private
requirement or machine accessible to its author under the
[private-issuer rule](../resources/authority.md#private-issuer-routes).
The catalog retains exact identities and dependencies for verification without
making the issuer callable, externally implementable, or source-nameable.
Public predicates, carrier types, and ordinary signatures keep their existing
exposure rules. Lexical subjects/binders do not become declaration-selection rows.

Value calls, Unit/discarded calls, static arguments, constructor fields, member
projections, and intrinsic unary expressions retain independent selections.
Computed receivers and normalization cannot erase their authored selections;
the selected field must agree with the checked projection. Constants retain
declaration provenance after value substitution. An intrinsic receives its
closed checked identity, not a fictional package owner.

Quotient formation retains carrier, relation, trait/application, and proof-
conformance selections. The relation and trait inherit the data interface's
exposure. The selected equivalence proof is private formation custody, absent
from mathematical API identity but still subject to visibility and dependencies.

## Admit before execution

Capture exact authored selections before source relationships are erased;
finalize late receiver/overload/conformance choices after checking. Missing,
ambiguous, or unjoinable package-source choices reject. Final execution consumes
the finalized selections. Earlier effect-free evaluation requires exact admitted
targets, or proof that the complete candidate set stays within admitted owners.
An implementation unable to establish that gate must reorder or split its work,
not execute first and check authority afterward.

Admission walks both caller-to-callee edges and selections in reachable bodies.
Shared evaluation policy is reusable only after every authored application site
is admitted. Literal/builtin spelling does not substitute for a checked intrinsic
identity. Carried semantic dependencies and authored selections are separate
compiler-private facts; neither requires another public IR stage.

Packages normally compose statically and can optimize across package boundaries.
They are not ABI or runtime replacement units by default. Independent realization
and frozen replacement envelopes follow
[component publication](../build/component_publication.md).
