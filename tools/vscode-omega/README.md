# Omega syntax highlighting

Local VS Code highlighting for `.omg` files: keywords, machine/state names, types,
constants, numbers, strings/escapes, line comments and nested block comments.
Colors follow your theme. It also adds comment shortcuts and bracket matching.
There is no language server, compiler dependency or background process.

## Install locally

With Node.js 20 or newer and VS Code installed, run from this directory on
Windows, macOS or Linux:

```sh
npx --yes @vscode/vsce@4.0.0 package --no-dependencies --out omega-syntax-0.1.2.vsix
code --install-extension omega-syntax-0.1.2.vsix
```

On PowerShell, use `npx.cmd` and `code.cmd` if script execution is disabled.
Or choose **Extensions: Install from VSIX...** in VS Code and select the
generated file. No Marketplace account or publishing is needed.

Open an `.omg` file; the language mode at the bottom right should say **Omega**.
If an open window has not picked up the extension, run **Developer:
Reload Window**. If a workspace explicitly associates `*.omg` with another
language, remove that association or choose **Omega** from the language picker.

## Source

[package.json](package.json) registers the language and its
[TextMate grammar](syntaxes/omega.tmLanguage.json).
[language-configuration.json](language-configuration.json) controls comments and
brackets. The grammar only highlights tokens; it does not show that the source
compiles. For example, capitalized names are colored as types by convention;
resolving them needs a future language server.

To inspect a token in the editor, use **Developer: Inspect Editor Tokens and
Scopes**. When editing the grammar, check strings containing comment markers, nested
comments, numeric suffixes and code after an unfinished string.
