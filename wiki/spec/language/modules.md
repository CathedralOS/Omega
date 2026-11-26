# Modules, imports, and visibility

A `use` declaration selects one logical source file and binds a file-local alias.
The alias defaults to the filename stem. Write `|alias|::name` to select a
declaration from that file.

<a id="module-header-position"></a>
<a id="directory-namespaces"></a>

## File identity

A declaration is identified within its owning package by its full admitted
logical file path and its declaration identity within that file. Two files in
one directory may independently declare the same name. A file can be moved
between checkouts without changing its identity. Moving a public declaration
to another logical file changes its public address. Keep the public machine
at its published address when delegating its implementation to a helper.
No module header is needed.

Generated and handwritten files use their logical filenames in the package's
[frozen source view](../build/execution.md#generated-source-boundary).
Two different source units cannot occupy one logical filename in the same
package and source context.

## Names and imports

```text
import_decl    := "use" ("{" ROOT "}" "/")? file_path ("as" IDENT)? ";"
file_path      := (component "/")* stem ".omg"
file_qualified := "|" IDENT "|" "::" declaration
```

Resolve a relative `file_path` from the current package root, including in
a file in a subdirectory. In `use {geometry}/math/vector.omg as vec;`, the
active dependency scope supplies the root `geometry`. Local `geometry/x.omg`
and rooted `{geometry}/x.omg` address distinct sources. The toolchain supplies
`{omega}/language/core/` in both checked build and product contexts.
For example, `use {omega}/language/core/foundation.omg as proof;` exposes
`|proof|::Identity` and `|proof|::Type` through an ordinary file-local alias.
The [five mathematical declarations](../proofs/mathematical_bindings.md#importing-the-mathematical-foundation)
use this same lookup rule.

Match the slash-separated logical filename exactly. Components may contain a
hyphen. Empty components, `.`, `..`, absolute host paths and directory spellings
reject. The filename includes its final literal `.omg`. There is no basename
search or implied index file. A stem that is not a legal
namespace identifier requires an explicit legal alias, for example
`use math/asdf-whatever.omg as whatever;`. Never normalize a hyphen to an
underscore when binding an alias. Duplicate aliases in one file reject at
the imports with both sites identified. Distinct aliases of one file select
the same declarations.

Outside `use`, the tokens `| IDENT | ::` start an imported declaration path.
The prefix is valid in expressions, types, patterns, contracts and declaration
selectors. Further static selections, such as `|wire|::Packet::Header`, follow
the chosen declaration. `@|wire|::Packet` borrows that type. Generic machine
calls keep their ordinary `::<...>` spelling. Fields use dot selection.
Existing compound tokens retain their lexical meaning. Write
`flags | |wire|::MASK` for bitwise OR with an imported constant. Lexing
`flags||wire|::MASK` produces logical OR and the expression rejects.
Similarly, `offset + |limits|::BASE` keeps plus separate from `+|`.

Each import selects a file without executing code or acquiring dependencies.
Each stage resolves the aliases it needs for checking against its own
frozen files and package bindings. A required target that is missing or
names a directory rejects that stage. A well-formed import may remain
unresolved in stages that do not use it. Syntax and duplicate-alias checks
still apply in every parsed file.

At the end of the build, warn once for each import declaration unused by
every stage compiled in that invocation, even if its target is missing.
Count a use by a successor compiling the same declaration. Reporting the
warning requires no final lookup of the unused target.
[Checked Build stages](../build/scoped_execution.md#checked-build-stages)

Dependency authority is context-specific. Build entry/local-helper imports use
build dependencies; product and generated-product imports use ordinary product
dependencies. The same alias can have distinct meanings in those scopes, with
no fallback. A host library uses its own ordinary edges for implementation code.
[Scoped build execution](../build/scoped_execution.md#two-checked-contexts) defines
these checked occurrences; source acquisition alone grants neither scope.
Only designated toolchain
[product-reference operands](../build/scoped_execution.md#selecting-product-declarations-without-executing-them)
resolve product declarations during build execution. Host imports, ordinary
calls, and generic helpers do not acquire that exception.

The [package boundary](../packages/boundaries.md) distinguishes authored
selection from carrying a foreign nominal type through a declared dependency's
API. Carrying does not add a direct dependency or grant selection authority.

### Import scope and exposure

Same-file declarations are directly nameable. Each other source file, including
a sibling, requires its own import and qualifier. A file alias exposes the
target file’s top-level declarations under ordinary visibility: package-private
declarations are accessible within their package, and external callers require
a direct dependency and a public declaration. Nested declarations retain their
own scope. A file's imports never become its exports. Selecting an alias gives
no bare names, including bare foreign domains. Bare-directory imports, item
imports and backslash-qualified source paths reject.

### Foreign attached declaration paths

Select a foreign domain through the file declaring the attachment.

```omega
use {ui_policy}/screen/rules.omg as screen;
```

Here `|screen|::Point::OnScreen` selects the domain on the exact carrier
resolved in that declaration's context.
Importing the carrier's file alone does not expose domains defined elsewhere.
Public access and direct dependency requirements apply to both owners.
Naming a domain supplies no membership, minting authority or representation
access. Foreign qualifications can travel through an interface under ordinary
[package boundary rules](../packages/boundaries.md).

## Resolution

Resolve an explicit file prefix only in the current file's import aliases.
Resolve a bare name in its ordinary lexical role, including local and arm
bindings, parameters, same-file declarations and language-supplied names.
Bare lookup never searches imported files. An import alias may have the same
spelling as a value, data type or operation record. `|math|::clamp`,
`math.clamp` and `math::Member` select their respective owners before member
lookup. Apply ordinary overload rules within the selected owner.

Case members are not an additional bare-name resolution step. Value position
requires the carrier-qualified `Type::Case` path even when an expected type or
visible carrier would make one case appear likely. Lexical bindings keep their
ordinary meaning; the compiler does not reinterpret an unresolved single-segment
name as a case.

Ambiguous eligible members reject. Diagnostics identify the selected owner and
competing declarations.

### Builtin type names

Named decision: `role-separated-builtin-type-names`, ratified 2026-11-13.

Compiler-defined type names, such as `bool`, `f32`, `f64`, `addr` and the
integer family, exist only in the type role. The integer family is open-ended:
`i`, `u`, and every `i` or `u` followed by decimal digits, whether or not the
digits form a valid width. `u0` is a family spelling and an invalid type. A
value binding with the same spelling never hides one, and no declaration may
redefine one. What is decided is the role of the name being introduced, not its
letters. A declaration introducing a type or namespace name may not use a
builtin type spelling. Value and universe-level binders may use those spellings.

| Introduced name | May use a builtin type spelling |
| --- | --- |
| A value: local, machine or state parameter, field, pattern binding, value constant, generic value binder, machine, named implementation | Yes. It does not affect type lookup. |
| A universe level: a binder declared with `\|proof\|::Level` | Yes. It introduces a name in level expressions. |
| A type: data declaration, trait schema, type alias, generic type binder | No. It would compete with the builtin type, and rejects where it is introduced. |
| A namespace: file import alias | No, for the same reason. |

Each use resolves by its role, determined before any operation is chosen:

- A type annotation, cast target or type-qualified path reads the builtin
  type. A literal suffix such as `7u32` is lexical and unaffected by bindings.
- An expression operand reads a value binding. A builtin type never stands in
  for a missing value.
- A generic argument takes its role from the selected declaration's binder: a
  type binder reads a type and `const N: u64` reads a value. Lookup never tries
  alternative roles to find one that checks. A universe-level argument reads a
  level expression. In `<u: |proof|::Level, A: |proof|::Type<u>>`, the argument to
  `|proof|::Type` reads the bound level. Type lookup still applies the ordinary
  integer-family spelling and width rules.
- Where an operand could be either, as in a type equality, a value binding
  takes the value role and a type/value mixture rejects. With a parameter
  named `i32` in scope, `requires T == i32` rejects; the diagnostic names both
  meanings and suggests renaming the parameter or using a type alias.
- A domain head such as `domain u32::NonZero` selects the carrier type.
  A static machine call such as `u32::play()` can select a supplied
  operation record named `u32`. A scalar type cannot own that machine.
  [Static qualification](#module-and-operation-record-names) defines
  how the syntactic role determines eligibility.

```omega
machine scaled(i: u32, u8: u16) -> u32 {
    let i1: u64 = 1;          // A value named i1; the type i1 is unaffected.
    let wide: u32 = u8 as u32; // The value u8, cast to the type u32.
    i + wide
}

data u7 { value: u32; }        // Rejected: redefines a builtin type.
trait u32 { }                  // Rejected: introduces a competing type name.
machine i(value: u32) -> u32 { value } // Accepted: a callable value.
```

### Module and operation-record names

With `use audio/play.omg as audio;`, `|audio|::play()` calls the imported
machine and `audio::play()` calls the operation of a supplied record named
`audio`.

For `::`, determine which visible meanings of the prefix can serve the
syntactic role before looking up the member. In a named machine call,
only owners that can supply a machine are eligible. A data type cannot own
a machine. In a domain test or qualification the carrier and domain are
eligible in their respective positions.

```omega
data Audio { volume: u8; }
domain Audio::Audible requires self.volume > 0;
trait Player { machine play(volume: u8); }

machine preview<const Audio: Player>(track: Audio)
requires track in Audio::Audible
{
    Audio::play(track.volume);
}
```

The parameter annotation selects the data type. The precondition selects its domain. The call
selects the operation record. Declared [domain operators](domains.md#domain-composition-and-operators)
remain selectable by name, such as `Small::greater`.

With one eligible owner, look up the member on that owner and report a
missing member there. With none, reject the use. With several, report the
ambiguous prefix before inspecting member names, argument types or result
types. Ordinary overload resolution applies to the selected owner's machines.
[Cases, nested types and attached domains](data_and_literals.md#records-sums-and-nested-declarations)
share their carrier's member namespace.

## Visibility

Named decision: `public-visibility-spelling`, settled 2026-10-19.

Independently nameable data, domains, traits, machines,
wire schemas, operators, constants, and complete named conformances are private
unless marked `public`. Carrier qualification does not inherit visibility.
Fields, cases, [nested data declarations](data_and_literals.md#records-sums-and-nested-declarations),
and trait requirements instead inherit their exact
semantic owner's visibility. A data-attached domain head independently selects
its carrier and therefore needs authority to name that carrier. A machine head
cannot attach to a data carrier; its argument types and code-owning namespace
retain their ordinary independent declaration selections.

Nested machines are lexical helpers, not independently package-exportable
declarations. An enclosing machine's export does not export its descendants,
and qualification cannot bypass their lexical scope. Source nesting affects
visibility, not data ownership or implicit runtime arguments. See
[nested machines](machines.md#nested-machines-and-identity).

Ranking measures are private proof machinery; `public measure` rejects. A public
termination guarantee does not publish its implementation's ranking witness.
Requirement-local `as Name` labels have no standalone package visibility.
Public interfaces cannot select private declarations as consumer-nameable API.
An authorized public conformance may retain private implementation identities
without granting consumers direct selection of them. A public domain's
[issuer catalog](../resources/authority.md#private-issuer-routes) may likewise
retain private trait requirements or exact machines accessible to its author.
The same limited exception applies to exact reader/writer descriptors in its access policy.
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
invariants, borrowing, and qualification requirements, including
[domain-owned read/write policy](domains.md#domain-owned-read-and-write-authority).
Knowing a field name grants no access that its qualified type withholds. Those
restrictions are opt-in domain clauses, not field visibility or privileges for
every machine in a file, namespace, or package. Omitting them adds no access
restriction beyond ordinary validity, ownership, and other authority rules.
Visibility is not confidentiality, unforgeable authority, or an ABI policy.
There are no private fields, by design: access plans already authorize exact
operations on governed values, so field visibility would duplicate that
machinery to enforce nothing, and the shape of data is part of its API. A
consumer that uses only a type's operations rebuilds without edits when the
shape changes; a consumer that selects a field has taken that dependency on
purpose. Denying selection itself is a package-boundary property: selecting a
type's fields, cases or conformances requires a direct dependency on the owning
package, while carrying its values does not
([declaration selection](../packages/boundaries.md#exposure-and-declaration-selection)).

Constructing geometry or handle bits does not construct its routed authority or
provider provenance. Confidential state remains in custody outside the
observer's access. Changes to public shape change source/API identity;
independent binary compatibility and replacement follow explicit
[component contracts](../build/component_publication.md).
