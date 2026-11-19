# Syntax trees

Start at [syntax_trees.rs](syntax_trees.rs). `SyntaxTrees` owns source identity,
roots and tables. Their fields show the representation: declarations,
expressions, statements, type references and retained source text. Construction,
checkpoints, page adoption and root publication stay beside those structures.
[lib.rs](lib.rs) only exposes that owner to Rust callers.

| Follow the field or operation | Definition |
| --- | --- |
| `SyntaxTreeTables.items` | [declarations/items.rs](declarations/items.rs): item variants, their table and root handles; adjacent files own machines, data, domains, conformances, generic parameters, capabilities and proof declarations |
| `SyntaxTreeTables.expressions` | [expressions/values.rs](expressions/values.rs): expression rows and their table; [display.rs](expressions/display.rs) formats diagnostic names and [float_spelling.rs](expressions/float_spelling.rs) retains authored floats |
| `SyntaxTreeTables.statements` | [statements/control_flow.rs](statements/control_flow.rs): executable statements and transitions; [assembly.rs](statements/assembly.rs) owns assembly records |
| `SyntaxTreeTables.type_references` | [types/references.rs](types/references.rs): type syntax and constraints. [preparation.rs](types/preparation.rs) owns expanded occurrences, normalized constant operands and their declaration origins |
| `SyntaxTreeTables.text` | [text/owner.rs](text/owner.rs): retained source pages; [identifiers.rs](text/identifiers.rs) defines names and paths that refer to them |
| `ParsedSource`, `ParsedSources`, `ParsedUnit` | [files/parsed.rs](files/parsed.rs): source/root associations and standalone reusable parses |
| `ParsedSyntaxPages`, `ParsedSyntaxSource` | [files/pages.rs](files/pages.rs): published parser pages and their source associations |
| Copying between forests | [copying/items.rs](copying/items.rs) and [bodies.rs](copying/bodies.rs): declaration/body copying, preserving shared handles and text ownership |
| Copying between tables in one forest | [copying/statement_paths.rs](copying/statement_paths.rs): expression arguments and identifier paths retained by statements |
| Logical equality and page identity | [storage/equality.rs](storage/equality.rs), [relocation.rs](storage/relocation.rs), [paged_tables.rs](storage/paged_tables.rs) |
| Constant declarations | [constants.rs](declarations/constants.rs) keeps the parsed initializer. [constant_preparation.rs](declarations/constant_preparation.rs) records the materialized value and obligations for that occurrence when resolution needs them. |
| Diagnostic snapshots | [inspection/snapshot.rs](inspection/snapshot.rs): the snapshot representation; adjacent files expand the grammar domains |

The [parser](../../pipeline/01_tokens-to-syntax-trees/parser.rs) produces this
representation. [Symbol resolution](../../pipeline/02_syntax-trees-to-symbol-resolved-trees)
is its next consumer. Source spelling, syntax structure and storage identity
remain distinct; these trees do not claim that names, types or proofs are valid.

## Ownership

Generic handle/span arithmetic belongs to [foundation/arena](../../../foundation/arena/src/arena.rs).
Both individual and whole-span appends use its contiguous-span checks. Parser
code accesses the public table fields directly; it has no private span builder
or table-access wrapper layer. Tree copying belongs here and keeps the meaning
of each source and destination table explicit.

The paged-table macro, equality and relocation remain here because they apply
syntax-specific equality, source ownership and page-admission rules. Moving them
to foundation would make foundation depend on this representation. Keyword and
punctuation classification similarly belongs to the token vocabulary.

[tests/unit/](tests/unit/) contains the existing forest, node, text and snapshot
tests. No test bodies live in the representation files. Root exports retain the
public vocabulary used by compiler consumers; domain folders expose the code
that defines it.
