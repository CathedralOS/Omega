'use strict';

// Formats Omega (.omg) source text without invoking the compiler.
//
// Tokenizes the source using Omega's lexical rules (keywords, policy operators,
// nested block comments, lifetimes, `|alias|::` imports and `use` paths), then
// normalizes indentation, operator spacing, trailing whitespace and blank-line
// runs while preserving all comments, strings and line-break choices.
//
// Nothing here depends on VS Code, so tests can run it on plain strings.

const INDENT_CONTINUATION_KEYWORDS = new Set([
  'requires', 'ensures', 'reaches', 'crashes', 'terminates',
  'invariant', 'preserves', 'where',
]);

const CONTROL_PAREN_KEYWORDS = new Set([
  'as', 'crashes', 'else', 'ensures', 'if', 'in', 'invariant', 'match',
  'preserves', 'reaches', 'requires', 'return', 'terminates', 'transition',
  'when', 'while',
]);

const SPACE_BEFORE_BRACKET_KEYWORDS = new Set([
  'as', 'else', 'ensures', 'if', 'in', 'invariant', 'let', 'machine', 'match', 'mutable',
  'preserves', 'reaches', 'requires', 'return', 'satisfies', 'terminates',
  'transition', 'when', 'where', 'while', 'crashes',
]);

const DECLARATION_HEADER_KEYWORDS = new Set([
  'machine', 'state', 'data', 'trait', 'struct', 'domain', 'proposition',
  'measure', 'capability',
]);

const ATTRIBUTE_OWNER_KEYWORDS = new Set(['data', 'struct']);

const UNARY_PRECEDING_KEYWORDS = new Set([
  'as', 'by', 'case', 'crashes', 'else', 'ensures', 'if', 'in', 'invariant',
  'let', 'match', 'preserves', 'reaches', 'requires', 'return', 'satisfies',
  'terminates', 'transition', 'when', 'where', 'while',
]);

// Longest-match punctuation table matching
// source-rust/subsystems/compiler/source-to-psi/representations/tokens/lexemes/punctuation.rs
const MULTI_PUNCT = [
  '<<%', '<<|', '<<!', '..=',
  '+%', '+|', '+!', '+=',
  '-%', '-|', '-!', '->', '-=',
  '*%', '*|', '*!', '*=',
  '/%', '/|', '/!', '/=',
  '%%', '%|', '%!', '%=',
  '<<', '<=', '>=', '==', '!=', '&&', '||', '::', '..',
];

const BINARY_OPERATORS = new Set([
  '+', '+%', '+|', '+!', '+=',
  '-', '-%', '-|', '-!', '-=',
  '*', '*%', '*|', '*!', '*=',
  '/', '/%', '/|', '/!', '/=',
  '%', '%%', '%|', '%!', '%=',
  '<<', '<<%', '<<|', '<<!',
  '>>', '>>%', '>>|', '>>!',
  '=', '==', '!=', '<', '>', '<=', '>=',
  '&&', '||', '&', '|', '^', '->',
]);

const CONTINUATION_ENDING_PUNCT = new Set([
  ',', '=', '+=', '-=', '*=', '/=', '%=', '->',
  '+', '+%', '+|', '+!', '-', '-%', '-|', '-!',
  '*', '*%', '*|', '*!', '/', '/%', '/|', '/!',
  '%', '%%', '%|', '%!', '<<', '<<%', '<<|', '<<!',
  '>>', '>>%', '>>|', '>>!',
  '==', '!=', '<', '>', '<=', '>=', '&&', '||', '&', '|', '^',
]);

const IDENT_START = /[A-Za-z_]/;
const IDENT_PART = /[A-Za-z_0-9]/;
const DIGIT = /[0-9]/;

function isWordStart(ch) {
  return ch !== undefined && IDENT_START.test(ch);
}

function isWordPart(ch) {
  return ch !== undefined && IDENT_PART.test(ch);
}

function isDigit(ch) {
  return ch !== undefined && DIGIT.test(ch);
}

// Lexes Omega source into a flat stream of tokens including newlines and comments.
// Horizontal whitespace is omitted so the formatter can emit canonical spacing.
function tokenize(text) {
  const tokens = [];
  let index = 0;
  const length = text.length;
  let expectingUsePath = false;

  while (index < length) {
    const ch = text[index];
    const next = text[index + 1];

    // Newlines
    if (ch === '\r' || ch === '\n') {
      const start = index;
      if (ch === '\r' && next === '\n') index += 2;
      else index += 1;
      tokens.push({ kind: 'newline', text: text.slice(start, index), start, end: index });
      continue;
    }

    // Horizontal whitespace
    if (ch === ' ' || ch === '\t') {
      index += 1;
      while (index < length && (text[index] === ' ' || text[index] === '\t')) index += 1;
      continue;
    }

    // Line comments (`//` and `///`)
    if (ch === '/' && next === '/') {
      const start = index;
      index += 2;
      while (index < length && text[index] !== '\n' && text[index] !== '\r') index += 1;
      const raw = text.slice(start, index).replace(/[ \t]+$/, '');
      tokens.push({ kind: 'lineComment', text: raw, start, end: index });
      continue;
    }

    // Nested block comments (`/* ... /* ... */ ... */`)
    if (ch === '/' && next === '*') {
      const start = index;
      let depth = 0;
      while (index < length) {
        if (text[index] === '/' && text[index + 1] === '*') {
          depth += 1;
          index += 2;
        } else if (text[index] === '*' && text[index + 1] === '/') {
          depth -= 1;
          index += 2;
          if (depth === 0) break;
        } else {
          index += 1;
        }
      }
      tokens.push({ kind: 'blockComment', text: text.slice(start, index), start, end: index });
      continue;
    }

    // `use <path> [as <alias>];` path token so `{omega}/language/core/x.omg`
    // is never split around `{`, `}`, `/` or `.`.
    if (expectingUsePath) {
      expectingUsePath = false;
      const start = index;
      while (
        index < length &&
        text[index] !== ' ' &&
        text[index] !== '\t' &&
        text[index] !== '\r' &&
        text[index] !== '\n' &&
        text[index] !== ';'
      ) {
        index += 1;
      }
      if (index > start) {
        tokens.push({ kind: 'usePath', text: text.slice(start, index), start, end: index });
        continue;
      }
    }

    // String literals
    if (ch === '"') {
      const start = index;
      index += 1;
      while (index < length && text[index] !== '"' && text[index] !== '\n' && text[index] !== '\r') {
        if (text[index] === '\\' && index + 1 < length && text[index + 1] !== '\n' && text[index + 1] !== '\r') {
          index += 2;
        } else {
          index += 1;
        }
      }
      if (index < length && text[index] === '"') index += 1;
      tokens.push({ kind: 'string', text: text.slice(start, index), start, end: index });
      continue;
    }

    // Lifetimes (`'a`, `'storage`)
    if (ch === "'" && isWordStart(next)) {
      const start = index;
      index += 2;
      while (index < length && isWordPart(text[index])) index += 1;
      tokens.push({ kind: 'lifetime', text: text.slice(start, index), start, end: index });
      continue;
    }

    // Import alias (`|alias|` immediately followed by `::`)
    if (ch === '|' && next !== '|' && isWordStart(next)) {
      let scan = index + 2;
      while (scan < length && isWordPart(text[scan])) scan += 1;
      if (text[scan] === '|' && text[scan + 1] === ':' && text[scan + 2] === ':') {
        const start = index;
        index = scan + 1;
        tokens.push({ kind: 'alias', text: text.slice(start, index), start, end: index });
        continue;
      }
    }

    // Numeric literals (hex, binary, octal, decimal, floats, type suffixes)
    if (isDigit(ch)) {
      const start = index;
      if (ch === '0' && (next === 'x' || next === 'X' || next === 'b' || next === 'B' || next === 'o' || next === 'O')) {
        index += 2;
        while (index < length && /[0-9A-Fa-f_]/.test(text[index])) index += 1;
      } else {
        index += 1;
        while (index < length && /[0-9_]/.test(text[index])) index += 1;
        // Decimal fraction only when `.` is not `..` and followed by a digit.
        if (text[index] === '.' && text[index + 1] !== '.' && isDigit(text[index + 1])) {
          index += 1;
          while (index < length && /[0-9_]/.test(text[index])) index += 1;
        }
        // Exponent (`e10`, `e-3`, `E+2`)
        if ((text[index] === 'e' || text[index] === 'E') && (isDigit(text[index + 1]) || ((text[index + 1] === '+' || text[index + 1] === '-') && isDigit(text[index + 2])))) {
          index += 1;
          if (text[index] === '+' || text[index] === '-') index += 1;
          while (index < length && /[0-9_]/.test(text[index])) index += 1;
        }
      }
      // Optional type suffix (`u64`, `i32`, `f64`, etc.)
      if (isWordStart(text[index])) {
        while (index < length && isWordPart(text[index])) index += 1;
      }
      tokens.push({ kind: 'number', text: text.slice(start, index), start, end: index });
      continue;
    }

    // Identifiers and keywords
    if (isWordStart(ch)) {
      const start = index;
      index += 1;
      while (index < length && isWordPart(text[index])) index += 1;
      const word = text.slice(start, index);
      const prevCode = previousCodeToken(tokens);
      if (word === 'use' && (!prevCode || prevCode.text === ';' || prevCode.text === '{' || prevCode.text === '}')) {
        expectingUsePath = true;
      }
      tokens.push({ kind: 'word', text: word, start, end: index });
      continue;
    }

    // Multi-character punctuation
    let matched = '';
    for (const candidate of MULTI_PUNCT) {
      if (text.startsWith(candidate, index)) {
        matched = candidate;
        break;
      }
    }
    if (matched) {
      const start = index;
      index += matched.length;
      tokens.push({ kind: 'punct', text: matched, start, end: index });
      continue;
    }

    // Single-character punctuation or unknown symbol
    const start = index;
    index += 1;
    tokens.push({ kind: 'punct', text: ch, start, end: index });
  }

  return tokens;
}

function previousCodeToken(tokens) {
  for (let i = tokens.length - 1; i >= 0; i -= 1) {
    const kind = tokens[i].kind;
    if (kind !== 'newline' && kind !== 'lineComment' && kind !== 'blockComment') {
      return tokens[i];
    }
  }
  return null;
}

// Annotates tokens with structural roles needed for spacing:
// - `genericOpen` / `genericClose` on `<` and `>` that delimit type/const parameters
// - merges touching `>` `>` (plus optional `%`/`|`/`!`) into right-shift tokens
//   when NOT closing a generic list
// - `unary` on `-` when used as unary negation
// - `attrOpen` on `[` when it opens a declaration attribute (`data Foo [linear];`)
function classifyTokens(rawTokens) {
  // Pre-pass: split `>=` into `>` + `=` when `>=` immediately closes a generic
  // argument list (e.g. `Vec<Option<u64>>=empty`).
  const expanded = expandGenericTrailingEquals(rawTokens);

  // First pass: identify generic `<` and `>` pairs on non-comment/newline code indices.
  const codeIndices = [];
  for (let i = 0; i < expanded.length; i += 1) {
    const k = expanded[i].kind;
    if (k !== 'newline' && k !== 'lineComment' && k !== 'blockComment') {
      codeIndices.push(i);
    }
  }

  const genericOpen = new Set();
  const genericClose = new Set();

  for (let ci = 0; ci < codeIndices.length; ci += 1) {
    const idx = codeIndices[ci];
    const tok = expanded[idx];
    if (tok.kind !== 'punct' || tok.text !== '<') continue;

    const prev = ci > 0 ? expanded[codeIndices[ci - 1]] : null;
    const prev2 = ci > 1 ? expanded[codeIndices[ci - 2]] : null;
    if (!couldOpenGeneric(prev, prev2, expanded, codeIndices, ci)) continue;

    const closeCi = findMatchingGenericClose(expanded, codeIndices, ci);
    if (closeCi >= 0) {
      genericOpen.add(idx);
      genericClose.add(codeIndices[closeCi]);
    }
  }

  // Second pass: copy tokens, merging touching right-shift operators when outside generics.
  const tokens = [];
  let i = 0;
  while (i < expanded.length) {
    const tok = expanded[i];
    if (
      tok.kind === 'punct' &&
      tok.text === '>' &&
      !genericClose.has(i) &&
      i + 1 < expanded.length &&
      expanded[i + 1].kind === 'punct' &&
      expanded[i + 1].text === '>' &&
      !genericClose.has(i + 1) &&
      tok.end === expanded[i + 1].start
    ) {
      // Check for policy suffix `%`, `|`, `!` touching the second `>`
      const third = expanded[i + 2];
      if (
        third &&
        third.kind === 'punct' &&
        (third.text === '%' || third.text === '|' || third.text === '!') &&
        expanded[i + 1].end === third.start
      ) {
        tokens.push({
          kind: 'punct',
          text: '>>' + third.text,
          start: tok.start,
          end: third.end,
        });
        i += 3;
        continue;
      }
      tokens.push({
        kind: 'punct',
        text: '>>',
        start: tok.start,
        end: expanded[i + 1].end,
      });
      i += 2;
      continue;
    }

    const copy = { ...tok };
    if (genericOpen.has(i)) copy.role = 'genericOpen';
    else if (genericClose.has(i)) copy.role = 'genericClose';
    tokens.push(copy);
    i += 1;
  }

  // Third pass: mark unary `-`, declaration/generic parameter attributes (`[copy]`,
  // `[linear]`), and postfix `as` cast entry signs (`+Wrapping`, `-Km`, `=Valid`).
  let prevCode = null;
  let prev2Code = null;
  let genericDepth = 0;
  for (let k = 0; k < tokens.length; k += 1) {
    const tok = tokens[k];
    if (tok.kind === 'newline' || tok.kind === 'lineComment' || tok.kind === 'blockComment') {
      continue;
    }
    if (tok.role === 'genericOpen') {
      genericDepth += 1;
    } else if (tok.role === 'genericClose') {
      genericDepth = Math.max(0, genericDepth - 1);
    } else if (tok.kind === 'punct' && tok.text === '-') {
      if (isUnaryContext(prevCode)) {
        tok.role = 'unary';
      }
    } else if (tok.kind === 'punct' && tok.text === '[') {
      if (
        (genericDepth > 0 && prevCode && prevCode.kind === 'word') ||
        (prevCode &&
          prevCode.kind === 'word' &&
          prev2Code &&
          prev2Code.kind === 'word' &&
          ATTRIBUTE_OWNER_KEYWORDS.has(prev2Code.text))
      ) {
        tok.role = 'attrOpen';
      }
    }
    prev2Code = prevCode;
    prevCode = tok;
  }

  markCastEntrySigns(tokens);

  return tokens;
}

// Marks `+`, `-`, and `=` tokens that introduce domain entries in a postfix
// `as` cast (wiki/spec/language/domains.md#grouping):
//   `x as +Wrapping`
//   `((left as -Km -Wrapping) +% (right as -Km -Wrapping)) as +Km +Wrapping`
//   `x as u32 +Km -Old =Kept`
//   `sum as + |units|::u64::Distance`
function markCastEntrySigns(tokens) {
  const codeIndices = [];
  for (let i = 0; i < tokens.length; i += 1) {
    const k = tokens[i].kind;
    if (k !== 'newline' && k !== 'lineComment' && k !== 'blockComment') {
      codeIndices.push(i);
    }
  }

  const isCastSign = (tok) =>
    tok && tok.kind === 'punct' && (tok.text === '+' || tok.text === '-' || tok.text === '=');

  // Advances past one type or domain path starting at code position `ci`, returning
  // the next code position, or `ci` if no path starts here.
  const scanPath = (ci) => {
    let cur = ci;
    const at = (pos) => (pos < codeIndices.length ? tokens[codeIndices[pos]] : null);

    // Optional borrow prefix: `@['a] [mutable|write]`
    if (at(cur) && at(cur).kind === 'punct' && at(cur).text === '@') {
      cur += 1;
      if (at(cur) && at(cur).kind === 'lifetime') cur += 1;
      if (at(cur) && at(cur).kind === 'word' && (at(cur).text === 'mutable' || at(cur).text === 'write')) {
        cur += 1;
      }
    }

    const head = at(cur);
    if (!head) return ci;

    // Bracketed carrier type `[u8]`, `[u8; 4]` (may be followed by `::Domain`)
    if (head.kind === 'punct' && head.text === '[') {
      let depth = 0;
      while (cur < codeIndices.length) {
        const t = at(cur);
        if (t.kind === 'punct' && t.text === '[') depth += 1;
        else if (t.kind === 'punct' && t.text === ']') {
          depth -= 1;
          cur += 1;
          if (depth === 0) break;
          continue;
        }
        cur += 1;
      }
    } else if (head.kind === 'alias' || head.kind === 'word') {
      cur += 1;
    } else {
      return ci;
    }

    // Consume chained `::segment` and `<...>` generic arguments.
    while (cur < codeIndices.length) {
      const t = at(cur);
      if (t.kind === 'punct' && t.text === '::') {
        cur += 1;
        if (at(cur) && (at(cur).kind === 'word' || at(cur).kind === 'alias')) {
          cur += 1;
        } else if (at(cur) && at(cur).role === 'genericOpen') {
          let depth = 0;
          while (cur < codeIndices.length) {
            if (at(cur).role === 'genericOpen') depth += 1;
            else if (at(cur).role === 'genericClose') {
              depth -= 1;
              cur += 1;
              if (depth === 0) break;
              continue;
            }
            cur += 1;
          }
        }
      } else if (t.role === 'genericOpen') {
        let depth = 0;
        while (cur < codeIndices.length) {
          if (at(cur).role === 'genericOpen') depth += 1;
          else if (at(cur).role === 'genericClose') {
            depth -= 1;
            cur += 1;
            if (depth === 0) break;
            continue;
          }
          cur += 1;
        }
      } else {
        break;
      }
    }
    return cur;
  };

  for (let ci = 0; ci < codeIndices.length; ci += 1) {
    const tok = tokens[codeIndices[ci]];
    if (tok.kind !== 'word' || tok.text !== 'as') continue;

    // Skip `use path as alias;`
    const prev = ci > 0 ? tokens[codeIndices[ci - 1]] : null;
    if (prev && prev.kind === 'usePath') continue;

    let cur = ci + 1;
    const at = (pos) => (pos < codeIndices.length ? tokens[codeIndices[pos]] : null);

    // Optional carrier type (including optional `in Domain [& Domain]*`).
    if (at(cur) && !isCastSign(at(cur))) {
      const afterType = scanPath(cur);
      if (afterType === cur) continue;
      cur = afterType;
      if (at(cur) && at(cur).kind === 'word' && at(cur).text === 'in') {
        cur += 1;
        cur = scanPath(cur);
        while (at(cur) && at(cur).kind === 'punct' && at(cur).text === '&') {
          cur += 1;
          cur = scanPath(cur);
        }
      }
    }

    // Longest run of `+D`, `-D`, `=D` domain cast entries.
    while (isCastSign(at(cur))) {
      const signTok = at(cur);
      const afterDomain = scanPath(cur + 1);
      if (afterDomain === cur + 1) break;
      signTok.role = 'castSign';
      cur = afterDomain;
    }
  }
}

function expandGenericTrailingEquals(rawTokens) {
  const codeIndices = [];
  for (let i = 0; i < rawTokens.length; i += 1) {
    const k = rawTokens[i].kind;
    if (k !== 'newline' && k !== 'lineComment' && k !== 'blockComment') {
      codeIndices.push(i);
    }
  }
  const splitAt = new Set();
  for (let ci = 0; ci < codeIndices.length; ci += 1) {
    const idx = codeIndices[ci];
    const tok = rawTokens[idx];
    if (tok.kind !== 'punct' || tok.text !== '<') continue;
    const prev = ci > 0 ? rawTokens[codeIndices[ci - 1]] : null;
    const prev2 = ci > 1 ? rawTokens[codeIndices[ci - 2]] : null;
    if (!couldOpenGeneric(prev, prev2, rawTokens, codeIndices, ci)) continue;

    let angleDepth = 1;
    let parenDepth = 0;
    let bracketDepth = 0;
    for (let scan = ci + 1; scan < codeIndices.length; scan += 1) {
      const t = rawTokens[codeIndices[scan]];
      if (t.kind !== 'punct') continue;
      if (t.text === '(') parenDepth += 1;
      else if (t.text === ')') {
        if (parenDepth === 0) break;
        parenDepth -= 1;
      } else if (t.text === '[') bracketDepth += 1;
      else if (t.text === ']') {
        if (bracketDepth === 0) break;
        bracketDepth -= 1;
      } else if (parenDepth === 0 && bracketDepth === 0) {
        if (t.text === '<') angleDepth += 1;
        else if (t.text === '>') {
          angleDepth -= 1;
          if (angleDepth === 0) break;
        } else if (t.text === '>=' && angleDepth === 1) {
          splitAt.add(codeIndices[scan]);
          break;
        } else if (
          t.text === ';' ||
          t.text === '{' ||
          t.text === '}' ||
          t.text === '&&' ||
          t.text === '||' ||
          t.text === '==' ||
          t.text === '!=' ||
          t.text === '<='
        ) {
          break;
        }
      }
    }
  }

  if (splitAt.size === 0) return rawTokens;
  const out = [];
  for (let i = 0; i < rawTokens.length; i += 1) {
    const tok = rawTokens[i];
    if (splitAt.has(i)) {
      out.push({ kind: 'punct', text: '>', start: tok.start, end: tok.start + 1 });
      out.push({ kind: 'punct', text: '=', start: tok.start + 1, end: tok.end });
    } else {
      out.push(tok);
    }
  }
  return out;
}

function couldOpenGeneric(prev, prev2, rawTokens, codeIndices, ci) {
  if (!prev) return false;
  // Turbofish: `foo::<T>`
  if (prev.kind === 'punct' && prev.text === '::') return true;
  if (prev.kind !== 'word') return false;
  // Declaration header, including qualified names and operator tokens:
  // `machine foo<T>`, `machine Foo::bar<T>`, `machine [] read<T>`, `machine + add<T>`.
  for (let back = ci - 2; back >= Math.max(0, ci - 12); back -= 1) {
    const t = rawTokens[codeIndices[back]];
    if (t.kind === 'word' && DECLARATION_HEADER_KEYWORDS.has(t.text)) return true;
    if (t.kind === 'punct' && (t.text === ';' || t.text === '{' || t.text === '}' || t.text === '(' || t.text === ')')) {
      break;
    }
  }
  // PascalCase type name: `Vec<u32>`, `Source<'storage>`
  if (/^[A-Z]/.test(prev.text)) return true;
  return false;
}

function findMatchingGenericClose(rawTokens, codeIndices, openCi) {
  let angleDepth = 1;
  let parenDepth = 0;
  let bracketDepth = 0;
  for (let ci = openCi + 1; ci < codeIndices.length; ci += 1) {
    const tok = rawTokens[codeIndices[ci]];
    if (tok.kind === 'punct') {
      if (tok.text === '(') parenDepth += 1;
      else if (tok.text === ')') {
        if (parenDepth === 0) return -1;
        parenDepth -= 1;
      } else if (tok.text === '[') bracketDepth += 1;
      else if (tok.text === ']') {
        if (bracketDepth === 0) return -1;
        bracketDepth -= 1;
      } else if (parenDepth === 0 && bracketDepth === 0) {
        if (tok.text === '<') angleDepth += 1;
        else if (tok.text === '>') {
          angleDepth -= 1;
          if (angleDepth === 0) return ci;
        } else if (
          tok.text === ';' ||
          tok.text === '{' ||
          tok.text === '}' ||
          tok.text === '&&' ||
          tok.text === '||' ||
          tok.text === '==' ||
          tok.text === '!=' ||
          tok.text === '<=' ||
          tok.text === '>='
        ) {
          return -1;
        }
      }
    }
  }
  return -1;
}

function isUnaryContext(prev) {
  if (!prev) return true;
  if (prev.kind === 'word') return UNARY_PRECEDING_KEYWORDS.has(prev.text);
  if (prev.kind === 'punct') {
    if (prev.role === 'genericClose') return false;
    return (
      prev.text === '(' ||
      prev.text === '[' ||
      prev.text === '{' ||
      prev.text === ',' ||
      prev.text === ';' ||
      prev.text === ':' ||
      prev.text === '!' ||
      prev.text === '~' ||
      BINARY_OPERATORS.has(prev.text)
    );
  }
  return false;
}

// Decides whether a single space belongs between two adjacent tokens on the same line.
function needsSpace(prev, curr) {
  if (!prev || !curr) return false;

  // Comments on the same line always get a separating space.
  if (curr.kind === 'lineComment' || curr.kind === 'blockComment') return true;
  if (prev.kind === 'blockComment') {
    return !(curr.kind === 'punct' && (curr.text === ',' || curr.text === ';' || curr.text === ')' || curr.text === ']'));
  }

  const p = prev.text;
  const c = curr.text;

  // Empty delimiters: `()`, `[]`, `{}`
  if (p === '(' && c === ')') return false;
  if (p === '[' && c === ']') return false;
  if (p === '{' && c === '}') return false;

  // Path separator `::` never has surrounding spaces.
  if (p === '::' || c === '::') return false;

  // Member access `.` and ranges `..` / `..=` never have surrounding spaces.
  if (p === '.' || c === '.' || p === '..' || c === '..' || p === '..=' || c === '..=') return false;

  // Borrow `@` attaches directly to its target (`@x`, `@mutable x`, `@'a T`, `@[u8]`, `@|a|::T`).
  if (p === '@') return false;

  // Postfix `as` cast entry signs (`+Wrapping`, `-Km`, `=Valid`) attach to their
  // domain path, except before `|alias|::...` where `+ |a|::D` and `- |a|::D`
  // must keep a space so they do not lex as `+|` or `-|`.
  if (prev.role === 'castSign') {
    if (curr.kind === 'alias' || c.startsWith('|') || c.startsWith('%') || c.startsWith('!') || c.startsWith('=')) {
      return true;
    }
    return false;
  }
  if (curr.role === 'castSign') return true;

  // Unary `!`, `~`, `-` attach directly to their operand, EXCEPT when joining
  // `-` with `|alias|::...` (or another punctuation token) would lex as `-|`.
  if (p === '!' || p === '~') return false;
  if (prev.role === 'unary') {
    if (curr.kind === 'alias' || c.startsWith('|') || c.startsWith('%') || c.startsWith('!') || c.startsWith('=')) {
      return true;
    }
    return false;
  }

  // Generic `<` and `>`:
  if (prev.role === 'genericOpen') return false;
  if (curr.role === 'genericOpen') return false;
  if (curr.role === 'genericClose') return false;
  if (prev.role === 'genericClose') {
    if (curr.role === 'genericClose') return false;
    if (c === '(' || c === '[' || c === ',' || c === ';' || c === ')' || c === ']' || c === ':') return false;
    return true;
  }

  // Opening/closing parens & brackets:
  if (p === '(' || p === '[') return false;
  if (c === ')' || c === ']') return false;

  // Commas, semicolons, colons:
  if (c === ',' || c === ';' || c === ':') return false;
  if (p === ',' || p === ';' || p === ':') return true;

  // Inline braces:
  if (p === '{') return true;
  if (c === '}') return true;
  if (c === '{') {
    if (p === '(' || p === '[' || p === '{') return false;
    return true;
  }
  if (p === '}') {
    if (c === '(' || c === '[' || c === ',' || c === ';' || c === ')' || c === ']' || c === '}') return false;
    return true;
  }

  // Binary operators always have spaces around them (note generic `<`/`>` already handled above).
  if (prev.kind === 'punct' && BINARY_OPERATORS.has(p)) return true;
  if (curr.kind === 'punct' && BINARY_OPERATORS.has(c)) return true;

  // Opening `(` after control keywords gets a space; after call/machine/case names no space.
  if (c === '(') {
    return prev.kind === 'word' && CONTROL_PAREN_KEYWORDS.has(p);
  }

  // Opening `[` after declaration attributes (`data Foo [linear]`), lifetimes
  // (`@'storage [u8]`) or keywords (`@mutable [T; N]`) gets a space; after
  // expressions/identifiers (`entries[0]`) no space.
  if (c === '[') {
    if (curr.role === 'attrOpen') return true;
    if (prev.kind === 'lifetime') return true;
    return prev.kind === 'word' && SPACE_BEFORE_BRACKET_KEYWORDS.has(p);
  }

  // Everything else between value-like tokens (words, numbers, strings, lifetimes, aliases, `@`, etc.) gets a space.
  return true;
}

// Formats a single line's token list (excluding leading/trailing whitespace and newlines).
function formatLineTokens(lineTokens) {
  let out = '';
  for (let i = 0; i < lineTokens.length; i += 1) {
    const tok = lineTokens[i];
    if (i > 0 && needsSpace(lineTokens[i - 1], tok)) {
      out += ' ';
    }
    out += tok.text;
  }
  return out;
}

// Splits tokens into per-line arrays, preserving line count 1-to-1 with source lines.
// Multi-line block comments are split across lines while retaining their internal lines.
function splitIntoLines(tokens) {
  const lines = [[]];
  for (const tok of tokens) {
    if (tok.kind === 'newline') {
      lines.push([]);
    } else if (tok.kind === 'blockComment' && tok.text.includes('\n')) {
      const parts = tok.text.split(/\r?\n/);
      lines[lines.length - 1].push({
        kind: 'blockComment',
        text: parts[0].replace(/[ \t]+$/, ''),
        multilineStart: true,
      });
      for (let p = 1; p < parts.length; p += 1) {
        lines.push([{
          kind: 'blockComment',
          text: parts[p].replace(/[ \t]+$/, ''),
          multilineContinuation: true,
        }]);
      }
    } else {
      lines[lines.length - 1].push(tok);
    }
  }
  return lines;
}

function isOpener(tok) {
  return tok.kind === 'punct' && (tok.text === '{' || tok.text === '(' || tok.text === '[');
}

function isCloser(tok) {
  return tok.kind === 'punct' && (tok.text === '}' || tok.text === ')' || tok.text === ']');
}

// Computes the formatted lines for the entire document, returning an array of
// formatted line strings (one per source line, before blank-line collapsing).
function formatAllLines(text, indentUnit) {
  const rawTokens = tokenize(text);
  const tokens = classifyTokens(rawTokens);
  const sourceLines = splitIntoLines(tokens);

  const formattedLines = [];
  let blockDepth = 0;
  let inContinuation = false;
  let inHeaderOrClause = false;
  // Tracks whether we are inside a multi-line block comment and its base indent.
  let blockCommentBaseIndent = '';
  let blockCommentOriginalIndent = 0;

  const rawTextLines = text.split(/\r?\n/);

  for (let lineIdx = 0; lineIdx < sourceLines.length; lineIdx += 1) {
    const lineTokens = sourceLines[lineIdx];

    // Blank line
    if (lineTokens.length === 0) {
      formattedLines.push('');
      continue;
    }

    // Continuation line of a multi-line block comment: preserve relative indentation.
    if (lineTokens.length === 1 && lineTokens[0].multilineContinuation) {
      const rawLine = lineTokens[0].text;
      if (rawLine.trim().length === 0) {
        formattedLines.push('');
      } else {
        const leadingSpaces = rawLine.match(/^[ \t]*/)[0].length;
        const trimmed = rawLine.trimStart();
        if (trimmed.startsWith('*')) {
          formattedLines.push(blockCommentBaseIndent + ' ' + trimmed);
        } else {
          const extra = Math.max(0, leadingSpaces - blockCommentOriginalIndent);
          formattedLines.push(blockCommentBaseIndent + ' '.repeat(extra) + trimmed);
        }
      }
      continue;
    }

    const codeTokens = lineTokens.filter((t) => t.kind !== 'lineComment' && t.kind !== 'blockComment');
    const firstCode = codeTokens[0] || null;
    const lastCode = codeTokens.length > 0 ? codeTokens[codeTokens.length - 1] : null;

    // Count leading closers on this line that close outer scopes.
    let leadingClosers = 0;
    for (const tok of codeTokens) {
      if (isCloser(tok)) leadingClosers += 1;
      else break;
    }

    // Determine indentation level for this line.
    let lineDepth = Math.max(0, blockDepth - leadingClosers);

    // Contract clauses (`requires`, `ensures`, `reaches`, `crashes`, `terminates`,
    // `invariant`, `preserves`, `where`) align with the declaration at `blockDepth`.
    const startsClause = firstCode && firstCode.kind === 'word' && INDENT_CONTINUATION_KEYWORDS.has(firstCode.text);
    const startsBrace = firstCode && firstCode.kind === 'punct' && firstCode.text === '{';
    const startsArrow = firstCode && firstCode.kind === 'punct' && firstCode.text === '->';

    if (startsClause || startsBrace || leadingClosers > 0) {
      inContinuation = false;
    }
    if (startsClause) {
      inHeaderOrClause = true;
    }
    if (startsBrace || leadingClosers > 0) {
      inHeaderOrClause = false;
    }

    let extraIndent = 0;
    if (startsArrow || inContinuation) {
      extraIndent = 1;
    }

    // Standalone comment line inherits current blockDepth (and continuation if active).
    const indent = indentUnit.repeat(lineDepth + extraIndent);

    // Remember block comment baseline if this line starts a multiline block comment.
    if (lineTokens.some((t) => t.multilineStart)) {
      blockCommentBaseIndent = indent;
      const rawLine = rawTextLines[lineIdx] || '';
      blockCommentOriginalIndent = rawLine.match(/^[ \t]*/)[0].length;
    }

    formattedLines.push(indent + formatLineTokens(lineTokens));

    // Update blockDepth from net open/close brackets on this line.
    let netDelta = 0;
    for (const tok of codeTokens) {
      if (isOpener(tok)) netDelta += 1;
      else if (isCloser(tok)) netDelta -= 1;
    }
    blockDepth = Math.max(0, blockDepth + netDelta);

    // Update continuation state for the next line.
    if (codeTokens.length > 0) {
      const startsDecl =
        firstCode &&
        firstCode.kind === 'word' &&
        (firstCode.text === 'public' || firstCode.text === 'boundary' || DECLARATION_HEADER_KEYWORDS.has(firstCode.text));

      if (netDelta !== 0) {
        inContinuation = false;
        inHeaderOrClause = false;
      } else if (lastCode.kind === 'punct' && (lastCode.text === ';' || lastCode.text === '{' || lastCode.text === '}')) {
        inContinuation = false;
        inHeaderOrClause = false;
      } else if (
        lastCode.kind === 'word' &&
        INDENT_CONTINUATION_KEYWORDS.has(lastCode.text)
      ) {
        // Bare clause keyword (`requires\n    x < y`) indents the next line.
        inContinuation = true;
        inHeaderOrClause = true;
      } else if (
        lastCode.kind === 'punct' &&
        CONTINUATION_ENDING_PUNCT.has(lastCode.text)
      ) {
        // A trailing `,` inside a block (`transition` arms, record literals) separates
        // sibling entries; it only indents when continuing a declaration header or clause.
        if (lastCode.text !== ',' || inHeaderOrClause || startsDecl) {
          inContinuation = true;
        }
        if (startsDecl) inHeaderOrClause = true;
      } else if (startsDecl) {
        // Unterminated declaration header (`domain Foo::Bar\n    access Policy;`
        // or `machine foo(...)\n    -> Bar;`) continues on the next line unless
        // the next line starts a clause or `{`.
        inContinuation = true;
        inHeaderOrClause = true;
      }
    }
  }

  return formattedLines;
}

function detectEol(text) {
  return text.includes('\r\n') ? '\r\n' : '\n';
}

function makeIndentUnit(options = {}) {
  const tabSize = typeof options.tabSize === 'number' && options.tabSize > 0 ? options.tabSize : 4;
  const insertSpaces = options.insertSpaces !== false;
  return insertSpaces ? ' '.repeat(tabSize) : '\t';
}

// Formats a full Omega source document with canonical LF (`\n`) line endings.
function formatDocument(text, options = {}) {
  if (text.trim().length === 0) return '';
  const indentUnit = makeIndentUnit(options);
  const lines = formatAllLines(text, indentUnit);

  // Strip leading/trailing blank lines and collapse consecutive blank lines to at most one.
  let start = 0;
  while (start < lines.length && lines[start] === '') start += 1;
  let end = lines.length;
  while (end > start && lines[end - 1] === '') end -= 1;

  const collapsed = [];
  let prevBlank = false;
  for (let i = start; i < end; i += 1) {
    const isBlank = lines[i] === '';
    if (isBlank && prevBlank) continue;
    collapsed.push(lines[i]);
    prevBlank = isBlank;
  }

  return collapsed.join('\n') + '\n';
}

// Formats a 0-based inclusive line range `[startLine, endLine]` using full-document
// indentation context. Returns the formatted text for those lines (without trailing EOL).
function formatLineRange(text, startLine, endLine, options = {}) {
  const eol = detectEol(text);
  const indentUnit = makeIndentUnit(options);
  const lines = formatAllLines(text, indentUnit);
  const clampedStart = Math.max(0, Math.min(startLine, lines.length - 1));
  const clampedEnd = Math.max(clampedStart, Math.min(endLine, lines.length - 1));
  return lines.slice(clampedStart, clampedEnd + 1).join(eol);
}

module.exports = {
  formatDocument,
  formatLineRange,
  tokenize,
};
