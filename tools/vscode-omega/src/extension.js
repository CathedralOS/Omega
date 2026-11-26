'use strict';

// Answers Go to Definition (Ctrl+click, F12) for Omega names. Omega has no
// language server yet, so this searches source text for a declaration with
// the clicked spelling. The repository holds thousands of standalone samples
// that reuse names such as `Main`, so it searches the nearest scope first and
// stops at the first scope that declares the name:
//   1. a local `let`, parameter or generic parameter in an enclosing machine;
//   2. the current file;
//   3. the files its `use` lines name;
//   4. its package (the nearest folder with a `build.omg`);
//   5. the bundled library (`source/library`);
//   6. the whole workspace, only when nothing nearer declares the name.

const vscode = require('vscode');
const scan = require('./declarations');

// Folders never searched below a search root: version control, build output,
// the archive of retired code and the agent worktree pool (full copies of the
// repository).
const SKIPPED_FOLDERS = new Set(['.git', 'node_modules', 'target', '_recycling', '.agents']);
const decoder = new TextDecoder('utf-8');

// Parsed files, keyed by URI. An entry is reused while the file's
// modification time and size, or an open document's version, are unchanged.
const parsedFiles = new Map();
// Whether a path exists, keyed by URI; cleared when `.omg` files come or go.
const knownPaths = new Map();

function activate(context) {
  context.subscriptions.push(
    vscode.languages.registerDefinitionProvider({ language: 'omega' }, { provideDefinition }),
    vscode.workspace.onDidChangeWorkspaceFolders(() => knownPaths.clear()),
  );
  // Adding or removing a source or package file changes which paths exist.
  const watcher = vscode.workspace.createFileSystemWatcher('**/*.omg', false, true, false);
  watcher.onDidCreate(() => knownPaths.clear());
  watcher.onDidDelete(() => knownPaths.clear());
  context.subscriptions.push(watcher);
}

function deactivate() {
  parsedFiles.clear();
  knownPaths.clear();
}

async function provideDefinition(document, position, token) {
  const text = document.getText();
  const masked = scan.maskCommentsAndStrings(text);
  const identifier = scan.identifierAt(text, masked, document.offsetAt(position));
  if (!identifier) return undefined;
  const current = remember(document.uri, `v${document.version}`, text, masked);
  const reference = scan.referenceContext(masked, identifier.start, identifier.end);

  if (!reference.alias && !reference.qualifier && !reference.member) {
    const local = scan.findLocalBinding(masked, current.declarations, reference.name, identifier.start);
    if (local >= 0) {
      const start = document.positionAt(local);
      return new vscode.Location(document.uri, new vscode.Range(start, start.translate(0, reference.name.length)));
    }
  }

  // A qualified name keeps looking for a declaration with that qualifier,
  // but never walks the whole workspace once a nearer scope has the name.
  let nameOnly = null;
  const needsWorkspace = () => nameOnly === null;
  for await (const files of searchScopes(document.uri, current, reference, needsWorkspace)) {
    if (token.isCancellationRequested) return undefined;
    const found = { qualified: [], named: [] };
    for (const uri of files) {
      if (token.isCancellationRequested) return undefined;
      const parsed = await parse(uri, reference.name);
      if (!parsed) continue;
      const { matches, qualified } = scan.selectDeclarations(parsed.declarations, reference, stem(uri));
      for (const declaration of matches) (qualified ? found.qualified : found.named).push(location(uri, parsed, declaration));
    }
    if (found.qualified.length > 0) return unique(found.qualified);
    if (found.named.length > 0 && !nameOnly) {
      nameOnly = found.named;
      if (!reference.qualifier) break;
    }
  }
  return nameOnly ? unique(nameOnly) : undefined;
}

// Yields the files of each scope in search order. The current file comes
// first unless the click names an alias (`|abi|::plan`), whose file then leads.
async function* searchScopes(uri, current, reference, needsWorkspace) {
  const seen = new Set();
  const fresh = (uris) => uris.filter((item) => !seen.has(item.toString()) && seen.add(item.toString()));
  const folder = parent(uri);
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri);
  const packageRoot = await findPackageRoot(folder, workspaceFolder);
  const libraryRoot = await findLibraryRoot(folder);

  if (reference.alias) {
    const imported = [];
    for (const use of current.uses) {
      const target = scan.importTarget(use, reference);
      if (target) imported.push(...(await importedFiles(target, packageRoot || folder, libraryRoot)));
    }
    yield fresh(imported);
    return;
  }
  yield fresh([uri]);
  yield fresh(packageRoot ? await omegaFiles(packageRoot, true, true) : await omegaFiles(folder, false));
  if (libraryRoot) yield fresh(await omegaFiles(libraryRoot, true));
  if (!needsWorkspace()) return;
  const roots = workspaceFolder ? [workspaceFolder.uri] : (vscode.workspace.workspaceFolders || []).map((item) => item.uri);
  const everything = [];
  for (const root of roots) everything.push(...(await omegaFiles(root, true)));
  yield fresh(everything);
}

async function importedFiles(target, packageRoot, libraryRoot) {
  if (target.library) {
    return libraryRoot ? existing([vscode.Uri.joinPath(libraryRoot, target.library, target.file)]) : [];
  }
  if (!target.dependency) return existing([vscode.Uri.joinPath(packageRoot, target.file)]);
  for (const root of await dependencyRoots(packageRoot, target.dependency)) {
    const files = await existing([vscode.Uri.joinPath(root, target.file)]);
    if (files.length) return files;
  }
  return [];
}

// Finds the folders a dependency named `name` may live in: the `location`
// strings in the package's `build.omg`, the one ending in `name` first, then
// a sibling folder called `name`.
async function dependencyRoots(packageRoot, name) {
  if (!packageRoot) return [];
  const roots = [];
  try {
    const build = decoder.decode(await vscode.workspace.fs.readFile(vscode.Uri.joinPath(packageRoot, 'build.omg')));
    const locations = [...build.matchAll(/\blocation\s*:\s*"([^"]+)"/g)].map((match) => match[1]);
    const named = (location) => location.replace(/[\\/]+$/, '').split(/[\\/]/).pop() === name;
    for (const location of [...locations.filter(named), ...locations.filter((item) => !named(item))]) {
      roots.push(vscode.Uri.joinPath(packageRoot, location));
    }
  } catch {
    // A package without a readable build.omg has no declared dependencies.
  }
  roots.push(vscode.Uri.joinPath(packageRoot, '..', name));
  return existing(roots);
}

async function existing(uris) {
  const found = [];
  for (const uri of uris) if (await exists(uri)) found.push(uri);
  return found;
}

// The package root is the nearest folder holding `build.omg`, without leaving
// the workspace folder.
async function findPackageRoot(folder, workspaceFolder) {
  const limit = workspaceFolder ? workspaceFolder.uri.toString() : '';
  let cursor = folder;
  for (;;) {
    if (await exists(vscode.Uri.joinPath(cursor, 'build.omg'))) return cursor;
    if (!limit || cursor.toString() === limit) return null;
    const up = parent(cursor);
    if (up.toString() === cursor.toString() || !up.toString().startsWith(limit)) return null;
    cursor = up;
  }
}

// The bundled library is `source/library` in the checkout that holds the
// current file, so a worktree finds its own copy.
async function findLibraryRoot(folder) {
  const candidates = [];
  let cursor = folder;
  for (;;) {
    candidates.push(cursor);
    const up = parent(cursor);
    if (up.toString() === cursor.toString()) break;
    cursor = up;
  }
  for (const item of vscode.workspace.workspaceFolders || []) candidates.push(item.uri);
  for (const base of candidates) {
    const library = vscode.Uri.joinPath(base, 'source', 'library');
    if (await exists(vscode.Uri.joinPath(library, 'core'))) return library;
  }
  return null;
}

// Lists the `.omg` files under `root`. With `ownPackageOnly`, a subfolder
// holding its own `build.omg` is a separate package (the compiler's test
// fixtures, say) and is left out.
async function omegaFiles(root, recursive, ownPackageOnly = false) {
  const files = [];
  const pending = [root];
  while (pending.length > 0) {
    const folder = pending.pop();
    let entries;
    try {
      entries = await vscode.workspace.fs.readDirectory(folder);
    } catch {
      continue;
    }
    if (ownPackageOnly && folder !== root && entries.some(([name]) => name === 'build.omg')) continue;
    for (const [name, type] of entries) {
      if (type & vscode.FileType.Directory) {
        if (recursive && !SKIPPED_FOLDERS.has(name)) pending.push(vscode.Uri.joinPath(folder, name));
      } else if (name.endsWith('.omg')) {
        files.push(vscode.Uri.joinPath(folder, name));
      }
    }
  }
  return files;
}

// Parses a file, or returns null when it cannot declare `name` because the
// spelling never appears in it. Open documents are read from the editor so
// unsaved edits count.
async function parse(uri, name) {
  const key = uri.toString();
  const open = vscode.workspace.textDocuments.find((document) => document.uri.toString() === key);
  let stamp;
  if (open) {
    stamp = `v${open.version}`;
  } else {
    try {
      const stat = await vscode.workspace.fs.stat(uri);
      stamp = `${stat.mtime}:${stat.size}`;
    } catch {
      return null;
    }
  }
  const cached = parsedFiles.get(key);
  if (cached && cached.stamp === stamp) return cached.names.has(name) ? cached : null;
  let text;
  try {
    text = open ? open.getText() : decoder.decode(await vscode.workspace.fs.readFile(uri));
  } catch {
    return null;
  }
  // Most workspace files never mention the name; skip parsing them.
  if (!text.includes(name)) return null;
  const parsed = remember(uri, stamp, text, scan.maskCommentsAndStrings(text));
  return parsed.names.has(name) ? parsed : null;
}

function remember(uri, stamp, text, masked) {
  const declarations = scan.parseDeclarations(text, masked);
  const lineStarts = [0];
  for (let index = 0; index < text.length; index += 1) if (text[index] === '\n') lineStarts.push(index + 1);
  const parsed = {
    stamp,
    declarations,
    uses: scan.parseUses(masked),
    names: new Set(declarations.map((declaration) => declaration.name)),
    lineStarts,
  };
  parsedFiles.set(uri.toString(), parsed);
  return parsed;
}

function location(uri, parsed, declaration) {
  const starts = parsed.lineStarts;
  let low = 0;
  let high = starts.length - 1;
  while (low < high) {
    const middle = (low + high + 1) >> 1;
    if (starts[middle] <= declaration.offset) low = middle;
    else high = middle - 1;
  }
  const start = new vscode.Position(low, declaration.offset - starts[low]);
  return new vscode.Location(uri, new vscode.Range(start, start.translate(0, declaration.name.length)));
}

function unique(locations) {
  const seen = new Set();
  return locations.filter((item) => {
    const key = `${item.uri.toString()}#${item.range.start.line}:${item.range.start.character}`;
    return !seen.has(key) && seen.add(key);
  });
}

async function exists(uri) {
  const key = uri.toString();
  if (knownPaths.has(key)) return knownPaths.get(key);
  let found = false;
  try {
    await vscode.workspace.fs.stat(uri);
    found = true;
  } catch {
    found = false;
  }
  knownPaths.set(key, found);
  return found;
}

function parent(uri) {
  return vscode.Uri.joinPath(uri, '..');
}

function stem(uri) {
  const name = uri.path.slice(uri.path.lastIndexOf('/') + 1);
  return name.endsWith('.omg') ? name.slice(0, -4) : name;
}

module.exports = { activate, deactivate };
