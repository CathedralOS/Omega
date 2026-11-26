# Omega for VS Code

Local VS Code support for `.omg` files: syntax highlighting, and Go to
Definition (Ctrl+click or F12) on machines, data, cases, fields, traits,
domains, constants and local bindings. Go to Definition matches names by
reading source text, not by asking the compiler, so it can offer several
declarations that share a spelling. There is no language server, compiler
dependency or background process.

Highlighting covers keywords and contract clauses, machine/state names, types,
lifetimes, `@` borrows, `|alias|::` imports, constants, numbers,
strings/escapes, line comments and nested block comments. Colors follow your
theme. It also adds comment shortcuts and bracket matching.

## Install locally

With Node.js 20 or newer and VS Code installed, run from this directory on
Windows, macOS or Linux:

```sh
npx --yes @vscode/vsce@4.0.0 package --no-dependencies --out omega-syntax-0.3.0.vsix
code --install-extension omega-syntax-0.3.0.vsix
```

With Bun instead of Node.js, replace `npx --yes` with `bun x`. On PowerShell,
use `npx.cmd` and `code.cmd` if script execution is disabled. Or choose
**Extensions: Install from VSIX...** in VS Code and select the generated file.
No Marketplace account or publishing is needed.

Open an `.omg` file; the language mode at the bottom right should say **Omega**.
If an open window has not picked up the extension, run **Developer:
Reload Window**. If a workspace explicitly associates `*.omg` with another
language, remove that association or choose **Omega** from the language picker.

## Source

[package.json](package.json) registers the language, its
[TextMate grammar](syntaxes/omega.tmLanguage.json) and the
[extension entry](src/extension.js).
[language-configuration.json](language-configuration.json) controls comments and
brackets. The grammar only highlights tokens; it does not show that the source
compiles. For example, capitalized names are colored as types by convention.

### Go to Definition

The repository holds thousands of standalone samples that reuse names such as
`Main` and `entry_start`, so [extension.js](src/extension.js) searches the
nearest scope first and stops at the first one that declares the name:

1. a `let`, parameter or generic parameter of an enclosing machine;
2. the current file;
3. the files its `use` lines name, when a name starts with `|alias|::`. The alias selects
   the exact package-relative, `{dependency}/` or `{omega}/` file;
4. its package: the nearest folder with a `build.omg`, without nested packages
   such as test fixtures;
5. the bundled library in `source/library`;
6. the whole workspace, only when nothing nearer declares the name.

A qualified click such as `Activation::try_claim` or `integer::max` prefers a
declaration with that qualifier, owner or file name. Searches skip `.git`,
`node_modules`, `target`, `_recycling` and `.agents`. A name declared nowhere
reaches the whole-workspace search, which takes several seconds in the full
repository before VS Code reports no definition.

[declarations.js](src/declarations.js) does the text scan and has no VS Code
dependency. To test both files, run `bun test` (or `node --test`) here;
[fake-vscode.js](test/fake-vscode.js) stands in for the editor API.

The word lists follow the compiler, not a separate vocabulary. The lexer
reserves only the words in
[keywords.rs](../../source-rust/subsystems/compiler/source-to-psi/representations/tokens/lexemes/keywords.rs);
clause words such as `requires`, `crashes` and `terminates` are contextual, so
the parser accepts them as ordinary names too. The grammar therefore colors such a
word as a name when a call `(`, a field `:`, an assignment `=` or a member `.`
follows it, as in `state: StatePlan;` or `write(out, value)`. Builtin type names
and literal suffixes follow the
[lexical profile](../../wiki/spec/language/data_and_literals.md#lexical-profile)
and [builtin type names](../../wiki/spec/language/modules.md#builtin-type-names):
`bool`, `address`, `f32`, `f64` and any `i`/`u` width such as `u13`. Bare `i` and
`u` stay uncolored because value bindings often use those spellings. The
retired `dyn` spelling is marked deprecated.

To inspect a token in the editor, use **Developer: Inspect Editor Tokens and
Scopes**. When editing the grammar, check strings containing comment markers,
nested comments, numeric suffixes, code after an unfinished string, and
contextual words used as field or machine names.
