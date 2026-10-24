// build.ts. Renders the mini-book as a single index.html and stages the
// self-contained static bundle into dist/ (only the referenced HTML, CSS, JS,
// woff2 fonts, and WebP/PNG plates — no source masters or scratch).
import { chapters } from "./src/chapters";
import { mkdirSync, rmSync } from "fs";
import { dirname, join } from "path";

const esc = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

// Reader prose: escape, then set `backticked` spans in the code font.
const pro = (s: string) => esc(s).replace(/`([^`]+)`/g, "<code>$1</code>");

// Warm-palette syntax highlighting for Omega source lines. Diagnostic blocks
// (`verify: false`) stay plain ink so compiler output reads as terminal text.
const KW = new Set([
  "use", "data", "case", "machine", "state", "transition", "let", "mut",
  "public", "boundary", "trait", "reaches", "satisfies", "requires",
  "ensures", "where", "in", "as", "block", "suspend", "crashes",
]);
const TY = new Set([
  "u8", "u16", "u32", "u64", "i32", "i64", "bool", "Wrapping", "Saturating",
  "Trapping", "copy", "true", "false", "result", "Console", "ProcessExit",
  "Build", "Source", "ProgramEntry",
]);

function syn(raw: string, isDiag = false): string {
  if (isDiag) {
    if (raw.startsWith("error:")) {
      return `<span class="tk-kw">error:</span>${esc(raw.slice(6)).replace(/`([^`]+)`/g, `<span class="tk-ty">\`$1\`</span>`)}`;
    }
    return esc(raw).replace(/`([^`]+)`/g, `<span class="tk-ty">\`$1\`</span>`);
  }
  const ci = raw.indexOf("//");
  if (ci >= 0) {
    return `${syn(raw.slice(0, ci), false)}<span class="tk-cm">${esc(raw.slice(ci))}</span>`;
  }
  let out = "";
  let i = 0;
  while (i < raw.length) {
    if (raw[i] === '"') {
      let j = i + 1;
      while (j < raw.length && raw[j] !== '"') j++;
      out += `<span class="tk-st">${esc(raw.slice(i, j + 1))}</span>`;
      i = j + 1;
    } else if (raw[i] === ":" && raw[i + 1] === ":") {
      out += "::&#8203;";
      i += 2;
    } else if (/[A-Za-z_]/.test(raw[i])) {
      let j = i + 1;
      while (j < raw.length && /[A-Za-z0-9_]/.test(raw[j])) j++;
      const w = raw.slice(i, j);
      if (KW.has(w)) out += `<span class="tk-kw">${w}</span>`;
      else if (TY.has(w)) out += `<span class="tk-ty">${w}</span>`;
      else out += w;
      i = j;
    } else if (/[0-9]/.test(raw[i])) {
      let j = i + 1;
      while (j < raw.length && /[A-Za-z0-9_]/.test(raw[j])) j++;
      out += `<span class="tk-nu">${raw.slice(i, j)}</span>`;
      i = j;
    } else {
      out += esc(raw[i]);
      i++;
    }
  }
  return out;
}

// Read a WebP's true dimensions so every plate keeps its own aspect ratio;
// hardcoding one size is how plates get squeezed.
async function dims(src: string): Promise<string> {
  const f = Bun.file(src);
  if (!(await f.exists())) return "";
  const b = new Uint8Array(await f.arrayBuffer());
  if (b[0] !== 0x52 || b[8] !== 0x57) return ""; // not RIFF/WEBP; omit attrs
  const fmt = String.fromCharCode(b[12], b[13], b[14], b[15]);
  let w = 0, h = 0;
  if (fmt === "VP8X") {
    w = 1 + (b[24] | (b[25] << 8) | (b[26] << 16));
    h = 1 + (b[27] | (b[28] << 8) | (b[29] << 16));
  } else if (fmt === "VP8 ") {
    w = b[26] | ((b[27] & 0x3f) << 8);
    h = b[28] | ((b[29] & 0x3f) << 8);
  } else if (fmt === "VP8L") {
    const v = b[21] | (b[22] << 8) | (b[23] << 16) | (b[24] << 24);
    w = (v & 0x3fff) + 1;
    h = ((v >> 14) & 0x3fff) + 1;
  }
  return w && h ? `width="${w}" height="${h}"` : "";
}

function rail() {
  const items = [
    `<a href="#frontispiece"><span class="rnum">✦</span>Frontispiece</a>`,
    ...chapters.map(
      (c) =>
        `<a href="#${c.id}"><span class="rnum">${c.num}</span>${esc(c.title)}</a>`
    ),
    `<a href="#glossary"><span class="rnum">§</span>Glossary</a>`,
    `<a href="#endpaper"><span class="rnum">❦</span>Dramatis</a>`,
    `<a href="#colophon"><span class="rnum">❧</span>Colophon</a>`,
  ];
  return `<nav class="rail"><a class="rail-brand" href="#frontispiece">LEARN<br>OMEGA</a>${items.join("")}</nav>`;
}

function codeCard(
  code: string,
  highlight: number[] = [],
  keysByLine: Map<number, number[]> = new Map(),
  opts: { filename?: string; isDiag?: boolean } = {}
) {
  const rows = code.split("\n").map((line, i) => {
    const n = i + 1;
    const hl = highlight.includes(n) ? " hl" : "";
    const keys = keysByLine.get(n);
    const chips = keys
      ? `<span class="lnkeys">${keys.map((k) => `<span class="key">${k}</span>`).join("")}</span>`
      : "";
    const anno = keys ? " anno" : "";
    // Leading indentation becomes padding (in ch of the code font) rather than spaces,
    // so a narrow screen can't break a line inside its indent, and wrapped rows hang
    // under their own line instead of falling back to the card's left edge.
    const indent = line.length - line.trimStart().length;
    const body = line.slice(indent);
    // A key rides with the last token of its line so a wrap can't strand it
    // alone on a row; only the final word/punctuation is glued so longer paths
    // can still break cleanly at `::`.
    const glue = keys ? Math.max(body.trimEnd().length - 8, body.search(/[A-Za-z0-9_"')\];]+\s*$/)) : -1;
    const text = glue >= 0
      ? `${syn(body.slice(0, glue), opts.isDiag)}<span class="lntail">${syn(body.slice(glue), opts.isDiag)}${chips}</span>`
      : `${syn(body, opts.isDiag) || " "}${chips}`;
    const style = indent ? ` style="--indent:${indent}ch"` : "";
    return `<div class="ln${hl}${anno}" data-line="${n}"><span class="lnn">${n}</span><span class="lnt"${style}>${text}</span></div>`;
  });
  const head = opts.filename
    ? `<div class="code-head"><span class="code-file">${esc(opts.filename)}</span></div>`
    : opts.isDiag
      ? `<div class="code-head"><span class="code-file">omega --check</span></div>`
      : "";
  const kindCls = opts.isDiag ? " diag-card" : "";
  return `<div class="figure code-card${kindCls}">${head}<div class="rows">${rows.join("")}</div></div>`;
}

function tableCard(head: string[], rows: string[][]) {
  const h = `<tr>${head.map((c) => `<th>${pro(c)}</th>`).join("")}</tr>`;
  const b = rows
    .map((r) => `<tr>${r.map((c) => `<td>${pro(c)}</td>`).join("")}</tr>`)
    .join("");
  return `<div class="figure table-card"><table><thead>${h}</thead><tbody>${b}</tbody></table></div>`;
}

function proseCard(text: string) {
  return `<div class="figure prose-card">${pro(text)}</div>`;
}

function staff(s: (typeof chapters)[0]["staves"][0]) {
  // Each wired note gets a marginalia key: a circled numeral rendered at the
  // end of its code line and beside the note, joined across the gutter by a
  // subtle SVG leader curve on desktop.
  const keysByLine = new Map<number, number[]>();
  let k = 0;
  const noteKeys = s.notes.map((n) => {
    if (!n.for) return 0;
    k += 1;
    keysByLine.set(n.for, [...(keysByLine.get(n.for) ?? []), k]);
    return k;
  });
  const fig =
    s.figure.kind === "code"
      ? codeCard(s.figure.code, s.figure.highlight, keysByLine, {
          filename: s.figure.filename,
          isDiag: s.figure.verify === false,
        })
      : s.figure.kind === "table"
        ? tableCard(s.figure.head, s.figure.rows)
        : proseCard(s.figure.text);
  const notes = s.notes
    .map((n, i) => {
      const key = noteKeys[i];
      const mark = key ? `<span class="nkey">${key}</span>` : `<span class="ndot"></span>`;
      return `<div class="note"${n.for ? ` data-for="${n.for}"` : ""}>${mark}<p>${pro(n.text)}</p></div>`;
    })
    .join("");
  const wires = k > 0 ? `<svg class="wires" aria-hidden="true"></svg>` : "";
  return `<section class="staff">${s.title ? `<h3 class="staff-title">${esc(s.title)}</h3>` : ""}
    <div class="staff-grid">${fig}${wires}<div class="notes">${notes}</div></div></section>`;
}

const head = `<!doctype html><html lang="en"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Learn Omega: a field guide for the agent era</title>
<meta name="description" content="A small book that teaches you to read Omega programs and check an agent's work.">
<link rel="icon" type="image/png" href="assets/favicon.png">
<link rel="apple-touch-icon" href="assets/favicon-180.png">
<meta property="og:type" content="website">
<meta property="og:title" content="Learn Omega: a field guide for the agent era">
<meta property="og:description" content="Omega is a systems language in which every program is a state machine that the compiler double-checks. This book teaches you to read it.">
<meta property="og:image" content="assets/social-card.png">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="Learn Omega: a field guide for the agent era">
<meta name="twitter:description" content="Omega is a systems language in which every program is a state machine that the compiler double-checks. This book teaches you to read it.">
<meta name="twitter:image" content="assets/social-card.png">
<link rel="preload" href="fonts/source-serif-4-latin-400-normal.woff2" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="fonts/hanken-grotesk-latin-400-normal.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="styles.css">
</head><body>`;

const tail = `<script src="book.js"></script></body></html>`;

const html = `${head}
${rail()}
<main class="book">
  <header class="cover-page" id="frontispiece">
    <header class="breadcrumb">A FIELD GUIDE FOR THE AGENT ERA</header>
    <img class="vignette" src="assets/plates/om-vignette-v2.webp" ${await dims("assets/plates/om-vignette-v2.webp")} fetchpriority="high" alt="Om, a small terracotta machine under a watercolor glass dome">
    <div class="cover-body">
      <h1>Learn to read<br><em>Omega</em></h1>
      <p class="thesis">Omega is a proof-checked systems language whose programs are data-oriented state
      machines. There is no <code>if</code> statement, no loop keyword, and no implicit <code>self</code>:
      a program is explicit states, exhaustive transitions, fixed-layout data, and contracts the compiler
      refuses to guess.</p>
      <p class="thesis">In practice, an agent will write most of your Omega. Your job is to specify the
      state graph, data bounds, and service reach up front, and to know when to send the agent's work back.</p>
      <div class="toc">
        ${chapters
          .map(
            (c) =>
              `<a class="toc-row" href="#${c.id}"><span class="toc-num">${c.num}</span><span class="toc-title">${esc(c.title)}</span><span class="toc-stand">${pro(c.standfirst)}</span></a>`
          )
          .join("")}
      </div>
      <p class="coderef">Every code figure follows the Omega language guide and compiles under the reference
      compiler. Hover or click any circled number to trace the line its note explains.</p>
    </div>
  </header>
  ${(await Promise.all(chapters
    .map(
      async (c) => `<article class="chapter" id="${c.id}">
    <img class="vignette" src="assets/plates/vignette-${c.id}.webp" ${await dims(`assets/plates/vignette-${c.id}.webp`)} loading="lazy" decoding="async" alt="" aria-hidden="true">
    <header class="ch-head">
      <p class="breadcrumb">CHAPTER ${c.num}</p>
      <h2 class="ch-title">${esc(c.title)}</h2>
      <p class="standfirst">${pro(c.standfirst)}</p>
    </header>
    ${c.staves.map(staff).join("\n    ")}
  </article>`
    ))).join("\n  ")}
  <section class="endmatter" id="glossary">
    <header class="ch-head"><p class="breadcrumb">END MATTER</p><h2 class="ch-title">Glossary</h2><p class="standfirst">The core vocabulary used by the Omega language guide, specification, and compiler diagnostics.</p></header>
    <dl class="gloss">
      ${[
        ["machine", "A state machine with explicit inputs, an optional return type, and a declared service ceiling (`reaches`). Entry points, helpers, and loops are all machines; there is no implicit `self` receiver."],
        ["state", "A named internal control point inside a machine with its own parameter frontier. Jumping to a state transfers control without pushing a call frame."],
        ["transition", "The control-transfer statement that ends a setup block or state body. Arms test a subject or guard list and jump to a state or return a value."],
        ["guard", "A boolean condition or pattern on a `transition` arm. Guard facts propagate into the target state, and all arms together must be exhaustive."],
        ["data", "A nominal record or sum-type declaration with a fixed compile-time layout and no hidden fields or default initializers."],
        ["case", "One named variant of a `data` sum type (`case Red;` or `case Hit(value: i32)`). Inspected exhaustively through `transition` dispatch."],
        ["payload", "The fields carried by a `case` variant, optionally constrained by a case-local `where` clause (`case Hit(value: i32) where value in 0..=255;`)."],
        ["[copy]", "Marks a pure value type as unrestricted so passing or assigning it duplicates the bits instead of consuming the binding."],
        ["affine", "The default ownership multiplicity for `data`: an owned value may move at most once, after which the old binding holds no value."],
        ["reaches", "A machine's conservative external-service ceiling, written `reaches Console + ProcessExit`. Calling a service outside that ceiling is rejected."],
        ["boundary trait", "A portable external interface declared by machine signatures (`public boundary trait Clock`). Each member states its own `reaches` row and any `suspends`, `blocks`, or `crashes` envelope."],
        ["provider", "A stateless type whose `satisfies` machines implement a boundary trait for a target or test harness (`FakeClock::now satisfies Clock::now`)."],
        ["static boundary call", "Calling a boundary requirement directly by trait path (`Console::write_line(...)`); the build selects the concrete provider for each target."],
        ["domain", "A zero-cost static theory or policy attached to a value (`i32 in Wrapping`, `Player::Alive`), checked at compile time with no runtime tag."],
        ["arithmetic policy", "The overflow contract on an integer type. Plain integers are Exact (every operation must be proven in range); `in Wrapping`, `in Saturating`, and `in Trapping` select defined overflow behavior."],
        ["proof obligation", "A static requirement the compiler must discharge — array bounds, exact arithmetic range, exhaustive dispatch, or precondition entailment — before accepting the program."],
        ["requires / ensures", "Machine contract clauses: `requires` states preconditions the caller must establish; `ensures` states postconditions guaranteed about `result` on normal return."],
        ["build machine", "`machine build(builder: &mut Build)` in `build.omg`: ordinary checked Omega code that declares the package kind, dependencies (`Build::depend`), and per-target `ProgramEntry` bindings (`Build::bind`)."],
        ["ProcessExit", "The core boundary service (`ProcessExit::exit_process(code: i32)`) that ends the process domain with an explicit status code, separate from `Console`."],
      ]
        .map(([t, d]) => `<div class="gterm"><dt>${esc(t)}</dt><dd>${pro(d)}</dd></div>`)
        .join("\n      ")}
    </dl>
  </section>
  <figure class="dramatis" id="endpaper">
    <header class="ch-head"><p class="breadcrumb">DRAMATIS PERSONAE</p><h2 class="ch-title">The Cast</h2><p class="standfirst">In order of appearance: the Prover, the Provider, the Console, Om, and the Agent.</p></header>
    <img src="assets/plates/cast-sheet.webp" ${await dims("assets/plates/cast-sheet.webp")} loading="lazy" decoding="async" alt="Character lineup: the Prover owl, the badger Provider, the Console printer, Om, and the Agent">
  </figure>
  <section class="endmatter colophon" id="colophon">
    <header class="ch-head"><p class="breadcrumb">END MATTER</p><h2 class="ch-title">Colophon</h2></header>
    <p>Set in Source Serif 4 and Hanken Grotesk on warm watercolor paper, with Om and the chapter vignettes painted from the canonical cast sheet. Every code figure in this guide is verified against the Omega reference compiler (<code>bun run verify</code>) before publication.</p>
    <a class="stone" href="#frontispiece">↑ back to the frontispiece</a>
  </section>
</main>
${tail}`;

await Bun.write("index.html", html);

// Stage a clean deployable bundle in dist/: only index.html, styles.css,
// book.js, and the exact assets/ and fonts/ paths referenced by them.
const css = await Bun.file("styles.css").text();
const js = await Bun.file("book.js").text();
const refs = new Set(
  [...(html + "\n" + css + "\n" + js).matchAll(/(?:assets|fonts)\/[A-Za-z0-9_.\-\/]+/g)].map(
    (m) => m[0]
  )
);
rmSync("dist", { recursive: true, force: true });
mkdirSync("dist", { recursive: true });
await Bun.write("dist/index.html", html);
await Bun.write("dist/styles.css", css);
await Bun.write("dist/book.js", js);
let bytes = html.length + css.length + js.length;
for (const rel of refs) {
  const src = Bun.file(rel);
  if (!(await src.exists())) throw new Error(`missing referenced asset: ${rel}`);
  const out = join("dist", rel);
  mkdirSync(dirname(out), { recursive: true });
  await Bun.write(out, src);
  bytes += src.size;
}
console.log(
  `built index.html + dist/ (${chapters.length} chapters, ${refs.size + 3} files, ${(bytes / 1024).toFixed(0)} KB)`
);
