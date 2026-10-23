# Exact nominal declaration owner

A library Token retains its declaring owner when it reaches library::read.

Owning requirement: **MODULE-NAMESPACE-RESOLUTION**, default builds and anonymous
owners under [build declarations](../../../../wiki/spec/build/declarations.md#default-builds-and-anonymous-owner-identity)
and [module selection](../../../../wiki/spec/language/modules.md).

The operation consumes `library::Token` through `library::read`, which forwards
it through generic `identity<Token>` before reading its byte. The authored oracle
is native exit 0 and Terminal completion with Unit after observing byte 7.
Each reject twin changes only the Token import to the equal-shaped declaration
in `copied`; `read` stays selected from `library`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The CLI samples use ordinary declared library inputs. They exercise supplied
owners and run with `omega run --both`; they do not alone establish anonymous
capture. The `compiler::anonymous_module_owner` integration target copies the
same library declarations into a root with no Build file, checks it through the
loose public compiler route, and obtains its content-derived owner. It supplies
that exact owner and sealed source to all nine consumers through explicit graph
edges. Each consumer executes natively and from its reloaded canonical Terminal
artifact. The test also publishes the standalone library through ordinary
Terminal inspection, compares its exact declaration across relocation/reload,
and refuses substitution from a changed snapshot with equal-shaped declarations
under the same module spelling. No host path or display name supplies identity.

* Establishment: the captured source and completed empty default-build dependency
  join establish the anonymous owner before Product resolution interns keys.
* Identity and lifetime: exact snapshot bytes and canonical relative paths commit
  the source; supplied owners and core custody remain separate. The source view
  stays immutable throughout loading. Relocation does not change the owner.
* Composition: exact nominal Token identity survives all nine transport places,
  generic specialization and supplied dependency edges. Namespace and distinct
  anonymous-owner substitutions reject at the consuming call.
* Soundness: neither source ownership nor artifact identity grants dependency,
  visibility, package acceptance or execution authority. The consumer supplies
  explicit edges and independently checked ProgramEntry acceptance.
* Psi 05–07: the standalone artifact and consumer artifacts retain owner-qualified
  declarations and carriers; canonical reload preserves their checked identities.
* Omega 00–02: the host consumer uses normal provider/entry settlement and layouts.
* Omega 03–06: ordinary aggregate value transport, calls and return slots carry
  the byte; no witness-specific lowering is introduced.
* Omega 07–09: the native consumer executes the emitted host image and agrees
  with the reloaded Terminal artifact. All four hosted targets are bound; only available-host Linux x86-64
  execution is claimed here.

The nine pairs are pinned under `tests/omega/{pass,fail}/facts/` as
`nominal_declaration_owner_through_<place>`. Compiler-internal source binding,
canonical record ordering, full-capture changes and preservation of supplied/core
owners have focused tests in `package-compilation::anonymous_owner`.

This bounded advance covers genuinely missing/default Builds and leaves the
parent capability open: authored anonymous Build activations still need final
owner sealing after their nonempty dependency join, while preserving private
activation state. Named-only OCREQ V1, additional constant forms and the other
remaining MODULE-NAMESPACE-RESOLUTION requirements are not closed by this row.
