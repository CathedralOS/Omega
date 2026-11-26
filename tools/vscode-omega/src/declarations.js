'use strict';

// Finds where Omega names are declared by reading source text, so the editor
// can answer Go to Definition without the compiler. It matches spellings and
// scopes, not resolved symbols: two packages that both declare `Main` both
// match, and the caller decides which files to search first.
//
// Nothing here depends on VS Code, so tests can run it on plain strings.

const IDENTIFIER = '[A-Za-z_][A-Za-z_0-9]*';
const IDENTIFIER_START = /[A-Za-z_]/;
const IDENTIFIER_PART = /[A-Za-z_0-9]/;

// Words that introduce a named declaration. The lexer reserves only some of
// them; the rest are contextual, so `state: Plan` or `data(x)` stay ordinary
// names because a name must follow the word after whitespace. A qualifier may
// be a type, as in `public domain [u8]::Path`.
const DECLARATION = new RegExp(
  `\\b(machine|state|data|trait|domain|proposition|measure|case|const|struct|capability)` +
    `\\s+((?:(?:${IDENTIFIER}|\\[[^\\]\\n]*\\])\\s*::\\s*)*)(${IDENTIFIER})`,
  'g',
);
const IMPLEMENTATION = new RegExp(`^[ \\t]*(?:public\\s+)?(${IDENTIFIER})\\s+satisfies\\b`, 'gmd');
const USE = new RegExp(`\\buse\\s+([^;\\s]+)(?:\\s+as\\s+(${IDENTIFIER}))?\\s*;`, 'g');
const RESERVED = new Set([
  'as', 'calling_convention', 'capability', 'contains', 'data', 'else', 'false',
  'foreign', 'host', 'if', 'let', 'library', 'loop', 'machine', 'match', 'owns',
  'platform', 'public', 'return', 'self', 'state', 'struct', 'target',
  'transition', 'true', 'use', 'when', 'while',
]);
// Declarations whose bodies name members: `case` variants, fields and the
// machines a trait requires.
const OWNER_KINDS = new Set(['data', 'trait', 'struct', 'domain']);
const SCOPE_KINDS = new Set(['machine', 'state']);

// Replaces comments and string contents with spaces. Offsets and line breaks
// stay where they were, so a match in the masked text is a position in the
// real text, and a name inside a comment never counts as a declaration.
function maskCommentsAndStrings(text) {
  const out = text.split('');
  let index = 0;
  const blank = (at) => {
    if (out[at] !== '\n' && out[at] !== '\r') out[at] = ' ';
  };
  while (index < text.length) {
    const char = text[index];
    const next = text[index + 1];
    if (char === '/' && next === '/') {
      while (index < text.length && text[index] !== '\n') blank(index++);
    } else if (char === '/' && next === '*') {
      // Block comments nest.
      let depth = 0;
      while (index < text.length) {
        if (text[index] === '/' && text[index + 1] === '*') {
          depth += 1;
          blank(index++);
          blank(index++);
        } else if (text[index] === '*' && text[index + 1] === '/') {
          depth -= 1;
          blank(index++);
          blank(index++);
          if (depth === 0) break;
        } else {
          blank(index++);
        }
      }
    } else if (char === '"') {
      // An unfinished string ends at the line break, so one typo does not
      // hide the rest of the file.
      index += 1;
      while (index < text.length && text[index] !== '"' && text[index] !== '\n') {
        if (text[index] === '\\' && index + 1 < text.length && text[index + 1] !== '\n') blank(index++);
        blank(index++);
      }
      index += 1;
    } else {
      index += 1;
    }
  }
  return out.join('');
}

function previousNonSpace(masked, offset) {
  let index = offset - 1;
  while (index >= 0 && /\s/.test(masked[index])) index -= 1;
  return index >= 0 ? masked[index] : '';
}

// Finds the `{` that opens a declaration's body, or -1 when the declaration
// ends with `;` first. Parentheses and generic brackets in the header do not
// contain `{`, `}` or `;`, so the first of those characters decides.
function bodyStart(masked, from) {
  for (let index = from; index < masked.length; index += 1) {
    const char = masked[index];
    if (char === '{') return index;
    if (char === ';' || char === '}') return -1;
  }
  return -1;
}

function matchingBrace(masked, open) {
  let depth = 0;
  for (let index = open; index < masked.length; index += 1) {
    if (masked[index] === '{') depth += 1;
    else if (masked[index] === '}') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return masked.length;
}

// Lists every declaration in one file. Each entry records the name, its
// explicit qualifier (`Activation` in `machine Activation::try_claim`) and the
// data or trait that owns it (`Attempt` for `case Rejected` inside
// `data Attempt { ... }`), so a qualified click can prefer the right one.
function parseDeclarations(text, masked = maskCommentsAndStrings(text)) {
  const declarations = [];
  const owners = [];
  DECLARATION.lastIndex = 0;
  let match;
  while ((match = DECLARATION.exec(masked)) !== null) {
    const kind = match[1];
    const qualifierText = match[2];
    const name = match[3];
    const start = match.index;
    const nameOffset = start + match[0].length - name.length;
    if (RESERVED.has(name) && name !== 'self') continue;
    // `const N: u64` inside `<...>` or `(...)` is a parameter, which only its
    // own machine or data can see.
    if (kind === 'const' && /[<,(]/.test(previousNonSpace(masked, start))) continue;
    const segments = qualifierText.split('::').map((part) => part.trim()).filter(Boolean);
    while (owners.length > 0 && owners[owners.length - 1].end < start) owners.pop();
    const owner = owners.length > 0 ? owners[owners.length - 1].name : '';
    declarations.push({
      kind,
      name,
      qualifier: segments.length > 0 ? segments[segments.length - 1] : '',
      owner,
      offset: nameOffset,
    });
    if (OWNER_KINDS.has(kind)) {
      const open = bodyStart(masked, DECLARATION.lastIndex);
      if (open >= 0) {
        const close = matchingBrace(masked, open);
        owners.push({ name, end: close });
        collectFields(masked, open, close, name, declarations);
      }
    }
  }
  // A named implementation starts its line with the name and no keyword:
  // `public VecCellLayout satisfies FixedLayout<VecCell> { ... }`.
  IMPLEMENTATION.lastIndex = 0;
  while ((match = IMPLEMENTATION.exec(masked)) !== null) {
    if (RESERVED.has(match[1])) continue;
    // A generic parameter list written one per line (`<\n Encode satisfies
    // Conversion,`) also starts lines this way; it follows `<` or `,`.
    if (/[<,(]/.test(previousNonSpace(masked, match.indices[1][0]))) continue;
    declarations.push({ kind: 'implementation', name: match[1], qualifier: '', owner: '', offset: match.indices[1][0] });
  }
  declarations.sort((left, right) => left.offset - right.offset);
  return declarations;
}

// Fields are `name: Type;` entries directly inside a data body. Nested braces
// belong to members (a trait machine's body, say), so a `;` counts only at the
// body's own level, and each nested block starts a new entry once it closes.
function collectFields(masked, open, close, owner, declarations) {
  const field = new RegExp(`^\\s*(?:public\\s+)?(${IDENTIFIER})\\s*:(?!:)`, 'd');
  let depth = 0;
  let entryStart = open + 1;
  for (let index = open + 1; index < close; index += 1) {
    const char = masked[index];
    if (char === '{') depth += 1;
    else if (char === '}') {
      depth -= 1;
      if (depth === 0) entryStart = index + 1;
    } else if (char === ';' && depth === 0) {
      const found = field.exec(masked.slice(entryStart, index));
      // Field names may be keywords (`state: Plan;`), so no word is excluded.
      if (found) {
        declarations.push({ kind: 'field', name: found[1], qualifier: '', owner, offset: entryStart + found.indices[1][0] });
      }
      entryStart = index + 1;
    }
  }
}

// Lists `use` lines: the written path and the alias, if any.
function parseUses(masked) {
  const uses = [];
  USE.lastIndex = 0;
  let match;
  while ((match = USE.exec(masked)) !== null) uses.push({ path: match[1], alias: match[2] || '' });
  return uses;
}

// Describes what surrounds the clicked name: `|output|::WavOutput::write_wav`
// gives alias `output` and qualifier `WavOutput`; `main.buffer` marks a member.
function referenceContext(masked, start, end) {
  const name = masked.slice(start, end);
  const before = masked.slice(Math.max(0, start - 400), start);
  const chain = new RegExp(`(?:\\|(${IDENTIFIER})\\|\\s*::\\s*)?((?:${IDENTIFIER}\\s*::\\s*)*)$`).exec(before);
  const segments = (chain[2] || '').split('::').map((part) => part.trim()).filter(Boolean);
  const member = /\.\s*$/.test(before) && !/\.\.\s*$/.test(before);
  return {
    name,
    alias: chain[1] || '',
    qualifier: segments.length > 0 ? segments[segments.length - 1] : '',
    member,
  };
}

// Returns the identifier span at an offset, or null when the offset is in a
// comment, a string or a reserved word.
function identifierAt(text, masked, offset) {
  let start = offset;
  let end = offset;
  while (start > 0 && IDENTIFIER_PART.test(masked[start - 1])) start -= 1;
  while (end < masked.length && IDENTIFIER_PART.test(masked[end])) end += 1;
  if (start === end || !IDENTIFIER_START.test(masked[start])) return null;
  // A lifetime such as `'storage` is not a name the scan can find.
  if (masked[start - 1] === "'") return null;
  const name = text.slice(start, end);
  // A keyword after `.` is a field such as `plan.state`.
  if (RESERVED.has(name) && previousNonSpace(masked, start) !== '.') return null;
  return { start, end, name };
}

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// Finds a local binding for `name` visible at `offset`: a `let` earlier in an
// enclosing machine or state body, or a parameter or generic parameter in its
// header. Searches from the innermost enclosing machine outward. Returns the
// offset of the binding's name, or -1.
function findLocalBinding(masked, declarations, name, offset) {
  const quoted = escapeRegExp(name);
  // The `d` flag reports where the captured name starts.
  const letPattern = new RegExp(`\\blet\\s+(?:mutable\\s+)?(${quoted})\\b`, 'gd');
  const headerPattern = new RegExp(`[(<,]\\s*(?:const\\s+)?(?:mutable\\s+)?(${quoted})\\s*[:,>)]`, 'gd');
  for (const open of enclosingBraces(masked, offset)) {
    const scope = scopeOwning(masked, declarations, open);
    if (!scope) continue;
    let last = -1;
    letPattern.lastIndex = open;
    let match;
    while ((match = letPattern.exec(masked)) !== null && match.index < offset) last = match.indices[1][0];
    if (last >= 0) return last;
    headerPattern.lastIndex = scope.offset;
    match = headerPattern.exec(masked);
    if (match && match.index < open) return match.indices[1][0];
  }
  return -1;
}

// Lists the `{` positions that are still open at `offset`, innermost first.
function enclosingBraces(masked, offset) {
  const braces = [];
  let balance = 0;
  for (let index = offset - 1; index >= 0; index -= 1) {
    const char = masked[index];
    if (char === '}') balance += 1;
    else if (char === '{') {
      if (balance === 0) braces.push(index);
      else balance -= 1;
    }
  }
  return braces;
}

// Returns the machine or state declaration whose body opens at `open`.
function scopeOwning(masked, declarations, open) {
  for (let index = declarations.length - 1; index >= 0; index -= 1) {
    const declaration = declarations[index];
    if (declaration.offset >= open) continue;
    if (!SCOPE_KINDS.has(declaration.kind)) return null;
    return bodyStart(masked, declaration.offset) === open ? declaration : null;
  }
  return null;
}

// Keeps the declarations that `reference` can mean. When the reference is
// qualified, declarations with that qualifier or owner, or in a file named
// after it (`integer::max` lives in `integer.omg`), come first; otherwise the
// name alone decides. `qualified` reports which rule chose the result.
function selectDeclarations(declarations, reference, fileStem) {
  const named = declarations.filter((declaration) =>
    declaration.name === reference.name && (reference.member ? declaration.kind === 'field' : true),
  );
  if (!reference.qualifier) return { matches: named, qualified: false };
  const qualifier = reference.qualifier;
  const qualified = named.filter(
    (declaration) => declaration.qualifier === qualifier || declaration.owner === qualifier || fileStem === qualifier,
  );
  return qualified.length > 0 ? { matches: qualified, qualified: true } : { matches: named, qualified: false };
}

// Says where a `use` line points, for a reference that may come from it.
// Returns null when the line cannot be where the name comes from.
//   `use calling.omg as abi;`              -> { file: 'calling.omg' }
//   `use {omega}/language/core/x.omg;`     -> { library: 'core', file: 'x.omg' }
//   `use {system}/x.omg;`                  -> { dependency: 'system', file: 'x.omg' }
//   `use {wav_output}/w/w.omg as output;`  -> { dependency: 'wav_output', file: 'w/w.omg' }
//   `use omega\language\core\Extent;`      -> { library: 'core' }
//   `use system\Console;`                  -> { module: 'system', dependency: 'system', inside: '' }
//   `use actors\state\Mood;`               -> { module: 'actors/state', dependency: 'actors', inside: 'state' }
// A `.omg` path is relative to the importing file's package, so the caller
// tries each folder from the file's own up to the package root. `{name}` is a
// dependency package that `build.omg` locates. A module path is either inside
// this package or, when its first segment names a dependency, inside that
// one. A path of module names imports one name, so it applies only when that
// name (or the clicked name's qualifier) is one of its segments.
function importTarget(use, reference) {
  const path = use.path;
  // A file imported without `as` is reached through its file name:
  // `use a/syntax_trees.omg;` lets the file write `|syntax_trees|::Kind`.
  const alias = use.alias || (/\.omg$/.test(path) ? path.slice(path.lastIndexOf('/') + 1, -4) : '');
  if (reference.alias && alias !== reference.alias) return null;
  if (/\.omg$/.test(path)) {
    const braced = /^\{([^}]+)\}\/(.+)$/.exec(path);
    if (!braced) return { file: path };
    const [, dependency, file] = braced;
    if (dependency === 'omega') {
      const core = /^language\/core\/(.+)$/.exec(file);
      if (core) return { library: 'core', file: core[1] };
      if (file.startsWith('build/')) return { library: 'build', file: file.slice('build/'.length) };
    }
    return { dependency, file };
  }
  const segments = path.split(/\\|::/).filter(Boolean);
  if (!reference.alias && !segments.includes(reference.name) && !(reference.qualifier && segments.includes(reference.qualifier))) {
    return null;
  }
  if (segments[0] === 'omega' && segments[1] === 'language' && segments[2] === 'core') return { library: 'core' };
  // An aliased path names a module (`use kit\main as k;`); otherwise its
  // last segment is the imported name and the rest is the module.
  const modulePath = path.split('::')[0].split('\\').filter(Boolean);
  const module = reference.alias ? modulePath : modulePath.slice(0, -1);
  if (module.length === 0) return null;
  return { module: module.join('/'), dependency: module[0], inside: module.slice(1).join('/') };
}

module.exports = {
  findLocalBinding,
  importTarget,
  identifierAt,
  maskCommentsAndStrings,
  parseDeclarations,
  parseUses,
  referenceContext,
  selectDeclarations,
};
