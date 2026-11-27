'use strict';

// The slice of the VS Code API that src/extension.js uses, backed by the real
// file system, so tests can run the definition provider without an editor.

const fs = require('node:fs');
const path = require('node:path');

class Uri {
  constructor(fsPath) {
    this.fsPath = path.resolve(fsPath);
    this.path = this.fsPath.split(path.sep).join('/');
    this.scheme = 'file';
  }
  static file(fsPath) {
    return new Uri(fsPath);
  }
  static joinPath(base, ...parts) {
    return new Uri(path.join(base.fsPath, ...parts));
  }
  toString() {
    return `file://${this.path}`;
  }
}

class Position {
  constructor(line, character) {
    this.line = line;
    this.character = character;
  }
  translate(lines, characters) {
    return new Position(this.line + lines, this.character + characters);
  }
}

class Range {
  constructor(start, end) {
    this.start = start;
    this.end = end;
  }
}

class Location {
  constructor(uri, range) {
    this.uri = uri;
    this.range = range;
  }
}

const EndOfLine = { LF: 1, CRLF: 2 };

class TextEdit {
  constructor(range, newText, newEol) {
    this.range = range;
    this.newText = newText;
    this.newEol = newEol;
  }
  static replace(range, newText) {
    return new TextEdit(range, newText, undefined);
  }
  static setEndOfLine(eol) {
    return new TextEdit(null, '', eol);
  }
}

const FileType = { File: 1, Directory: 2 };

class TextDocument {
  constructor(uri, text) {
    this.uri = uri;
    this.text = text;
    this.version = 1;
    this.eol = text.includes('\r\n') ? EndOfLine.CRLF : EndOfLine.LF;
    this._lines = text.split('\n');
    this.lineCount = this._lines.length;
  }
  lineAt(line) {
    const raw = this._lines[line] ?? '';
    const text = raw.endsWith('\r') ? raw.slice(0, -1) : raw;
    return {
      lineNumber: line,
      text,
      range: new Range(new Position(line, 0), new Position(line, text.length)),
    };
  }
  getText(range) {
    if (!range) return this.text;
    return this.text.slice(this.offsetAt(range.start), this.offsetAt(range.end));
  }
  offsetAt(position) {
    const lines = this.text.split('\n');
    let offset = 0;
    for (let line = 0; line < position.line; line += 1) offset += lines[line].length + 1;
    return offset + position.character;
  }
  positionAt(offset) {
    const before = this.text.slice(0, offset).split('\n');
    return new Position(before.length - 1, before[before.length - 1].length);
  }
}

function createVscode(workspaceRoots) {
  const folders = workspaceRoots.map((root, index) => ({ uri: Uri.file(root), index, name: path.basename(root) }));
  const notFound = () => Object.assign(new Error('not found'), { code: 'FileNotFound' });
  return {
    Uri,
    Position,
    Range,
    Location,
    EndOfLine,
    TextEdit,
    FileType,
    TextDocument,
    workspace: {
      workspaceFolders: folders,
      textDocuments: [],
      getWorkspaceFolder(uri) {
        return folders.find((folder) => (uri.fsPath + path.sep).startsWith(folder.uri.fsPath + path.sep));
      },
      fs: {
        async stat(uri) {
          try {
            const stat = fs.statSync(uri.fsPath);
            return { type: stat.isDirectory() ? FileType.Directory : FileType.File, mtime: stat.mtimeMs, size: stat.size };
          } catch {
            throw notFound();
          }
        },
        async readDirectory(uri) {
          try {
            return fs.readdirSync(uri.fsPath, { withFileTypes: true }).map((entry) => [entry.name, entry.isDirectory() ? FileType.Directory : FileType.File]);
          } catch {
            throw notFound();
          }
        },
        async readFile(uri) {
          try {
            return fs.readFileSync(uri.fsPath);
          } catch {
            throw notFound();
          }
        },
      },
      onDidChangeWorkspaceFolders: () => ({ dispose() {} }),
      createFileSystemWatcher: () => ({ onDidCreate() {}, onDidDelete() {}, dispose() {} }),
    },
    languages: {
      registerDefinitionProvider(selector, provider) {
        this.definitionProvider = provider;
        return { dispose() {} };
      },
      registerDocumentFormattingEditProvider(selector, provider) {
        this.formattingProvider = provider;
        return { dispose() {} };
      },
      registerDocumentRangeFormattingEditProvider(selector, provider) {
        this.rangeFormattingProvider = provider;
        return { dispose() {} };
      },
    },
  };
}

// Loads src/extension.js against a fake `vscode` and returns a function that
// opens `file` and asks for the definition at the `$` marker in `context`, a
// snippet of that file with one `$` inserted.
function loadExtension(workspaceRoots) {
  const vscode = createVscode(workspaceRoots);
  const extensionPath = require.resolve('../src/extension.js');
  delete require.cache[extensionPath];
  let extension;
  if (process.versions.bun) {
    // Bun resolves `require` without Node's module loader hook.
    require('bun:test').mock.module('vscode', () => vscode);
    extension = require(extensionPath);
  } else {
    const Module = require('node:module');
    const originalLoad = Module._load;
    Module._load = function load(request, ...rest) {
      return request === 'vscode' ? vscode : originalLoad.call(this, request, ...rest);
    };
    try {
      extension = require(extensionPath);
    } finally {
      Module._load = originalLoad;
    }
  }
  extension.activate({ subscriptions: [] });
  const provider = vscode.languages.definitionProvider;
  const definitionAt = async function definitionAt(file, context) {
    const marker = context.indexOf('$');
    const text = fs.readFileSync(file, 'utf8');
    const found = text.indexOf(context.slice(0, marker) + context.slice(marker + 1));
    if (found < 0) throw new Error(`context not in ${file}: ${context}`);
    const document = new TextDocument(Uri.file(file), text);
    vscode.workspace.textDocuments = [document];
    const result = await provider.provideDefinition(document, document.positionAt(found + marker), { isCancellationRequested: false });
    if (!result) return [];
    return (Array.isArray(result) ? result : [result]).map(describe);
  };
  definitionAt.format = (text, options = { tabSize: 4, insertSpaces: true }) => {
    const document = new TextDocument(Uri.file(path.join(workspaceRoots[0], 'sample.omg')), text);
    const edits = vscode.languages.formattingProvider.provideDocumentFormattingEdits(document, options);
    return applyEdits(document, edits);
  };
  definitionAt.formatRange = (text, startLine, endLine, options = { tabSize: 4, insertSpaces: true }) => {
    const document = new TextDocument(Uri.file(path.join(workspaceRoots[0], 'sample.omg')), text);
    const range = new Range(new Position(startLine, 0), new Position(endLine, document.lineAt(endLine).text.length));
    const edits = vscode.languages.rangeFormattingProvider.provideDocumentRangeFormattingEdits(document, range, options);
    return applyEdits(document, edits);
  };
  return definitionAt;
}

function applyEdits(document, edits) {
  let out = document.getText();
  let targetEol = document.eol;
  for (const edit of [...edits].reverse()) {
    if (edit.newEol !== undefined) {
      targetEol = edit.newEol;
      continue;
    }
    const start = document.offsetAt(edit.range.start);
    const end = document.offsetAt(edit.range.end);
    out = out.slice(0, start) + edit.newText + out.slice(end);
  }
  if (targetEol === EndOfLine.LF) out = out.replace(/\r\n/g, '\n');
  return out;
}

function describe(location) {
  const text = fs.readFileSync(location.uri.fsPath, 'utf8').split('\n')[location.range.start.line] || '';
  return { file: location.uri.fsPath, line: location.range.start.line + 1, text: text.trim() };
}

module.exports = { loadExtension };
