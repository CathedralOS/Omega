// book.js — the page's progressive JS. Without it the book is fully readable:
// circled keys on code lines match the numerals on their notes; the rail
// still jumps by anchor.
//
// 1. Note alignment + leader curves: each wired note is positioned so its key
//    sits level with the code row it annotates (absolute positioning in the
//    notes column with collision clamping), and an SVG Bezier curve bridges
//    the gutter between the code card and the note key. Staves are measured
//    lazily as they approach the viewport (content-visibility leaves
//    below-fold staves unlaid-out).
// 2. Hover + click-pin pairing: hovering or clicking a note, its code line, or
//    its key chip lights all three plus the connecting leader wire.
// 3. Scrollspy: marks the current chapter on the rail.

const staves = [...document.querySelectorAll(".staff-grid")];
const STACKED = "(max-width: 1100px)"; // matches the responsive breakpoint in styles.css
const NS = "http://www.w3.org/2000/svg";

function resetStaff(grid) {
  grid.querySelectorAll(".note").forEach((n) => {
    n.style.position = "";
    n.style.top = "";
    n.style.left = "";
    n.style.right = "";
  });
  const notesEl = grid.querySelector(".notes");
  if (notesEl) notesEl.style.height = "";
  const wires = grid.querySelector(".wires");
  if (wires) wires.replaceChildren();
}

function placeStaff(grid) {
  const figure = grid.querySelector(".figure");
  const notesEl = grid.querySelector(".notes");
  if (!figure || !notesEl) return;

  // Always reset before measuring so notes are measured in normal flow at their
  // current column width, not from stale absolute coordinates left by an
  // earlier pass.
  resetStaff(grid);

  // stacked layout: notes flow normally under the card
  if (window.matchMedia(STACKED).matches) return;

  const notes = [...grid.querySelectorAll(".note")];
  const nTop = notesEl.getBoundingClientRect().top;

  // measure while static; wired notes get a target line
  const items = notes.map((note) => {
    const row = note.dataset.for
      ? figure.querySelector(`[data-line="${note.dataset.for}"]`)
      : null;
    const mark = note.querySelector(".nkey") || note.querySelector(".ndot");
    const nr = note.getBoundingClientRect();
    const mr = mark ? mark.getBoundingClientRect() : null;
    const rr = row ? row.getBoundingClientRect() : null;
    return {
      note,
      row,
      mark,
      targetY: rr ? rr.top - nTop + rr.height / 2 : null,
      markOffset: mr ? mr.top - nr.top + mr.height / 2 : 14,
      height: nr.height,
    };
  });

  // place every note — wired ones at their line, unwired continue the stack
  // in document order; never overlap
  let prevBottom = -6;
  items.forEach((it) => {
    const want = it.targetY == null ? -6 : it.targetY - it.markOffset;
    const top = Math.max(want, prevBottom + 6);
    it.note.style.position = "absolute";
    it.note.style.top = `${top}px`;
    it.note.style.left = "0";
    it.note.style.right = "0";
    prevBottom = top + it.height;
  });
  notesEl.style.height = `${Math.max(prevBottom, 0)}px`;

  // draw SVG leader wires across the gutter between the code card's right edge
  // and each wired note's numeral badge
  const wires = grid.querySelector(".wires");
  if (!wires) return;
  const gr = grid.getBoundingClientRect();
  const fr = figure.getBoundingClientRect();
  const w = Math.max(gr.width, 1);
  const h = Math.max(gr.height, prevBottom, 1);
  wires.setAttribute("viewBox", `0 0 ${w} ${h}`);
  wires.style.width = `${w}px`;
  wires.style.height = `${h}px`;

  const x1 = fr.right - gr.left + 1;
  items.forEach((it) => {
    if (!it.row || !it.mark) return;
    const rr = it.row.getBoundingClientRect();
    const mr = it.mark.getBoundingClientRect();
    const y1 = rr.top - gr.top + rr.height / 2;
    const x2 = mr.left - gr.left - 3;
    const y2 = mr.top - gr.top + mr.height / 2;
    if (x2 <= x1 + 4) return;
    const dx = (x2 - x1) * 0.48;
    const d = `M ${x1.toFixed(1)} ${y1.toFixed(1)} C ${(x1 + dx).toFixed(1)} ${y1.toFixed(1)}, ${(x2 - dx).toFixed(1)} ${y2.toFixed(1)}, ${x2.toFixed(1)} ${y2.toFixed(1)}`;
    // Base dashed guide curve (always faintly visible)
    const base = document.createElementNS(NS, "path");
    base.setAttribute("class", "wire");
    base.setAttribute("d", d);
    // Drawn ink overlay: traces smoothly from code row to note on hover/pin
    const draw = document.createElementNS(NS, "path");
    draw.setAttribute("class", "wire-draw");
    draw.dataset.for = it.note.dataset.for;
    draw.setAttribute("d", d);
    draw.setAttribute("pathLength", "1");
    const dot = document.createElementNS(NS, "circle");
    dot.setAttribute("class", "wire-dot");
    dot.dataset.for = it.note.dataset.for;
    dot.setAttribute("cx", x1.toFixed(1));
    dot.setAttribute("cy", y1.toFixed(1));
    dot.setAttribute("r", "2.4");
    wires.append(base, draw, dot);
  });
}

const placeAll = () => staves.forEach(placeStaff);
const nearView = new IntersectionObserver(
  (es) => es.forEach((e) => e.isIntersecting && placeStaff(e.target)),
  { rootMargin: "300px 0px" }
);
staves.forEach((g) => nearView.observe(g));

placeAll();
if (document.fonts?.ready) document.fonts.ready.then(placeAll);
new ResizeObserver(placeAll).observe(document.querySelector(".book"));

// content-visibility placeholders shift anchor targets as staves lay out —
// re-settle the initial hash position once the page has real geometry.
if (location.hash.length > 1) {
  const settle = () => {
    try {
      document.getElementById(location.hash.slice(1))
        ?.scrollIntoView({ behavior: "instant", block: "start" });
    } catch {}
  };
  requestAnimationFrame(settle);
  if (document.fonts?.ready) document.fonts.ready.then(() => requestAnimationFrame(settle));
}

// ---- hover + click-pin pairing: note <-> code line <-> leader wire ----

let pinned = null;

function setLit(grid, line, on) {
  const figure = grid.querySelector(".figure");
  const row = figure?.querySelector(`[data-line="${line}"]`);
  const notes = grid.querySelectorAll(`.note[data-for="${line}"]`);
  const wires = grid.querySelectorAll(`.wires [data-for="${line}"]`);
  row?.classList.toggle("lit", on);
  notes.forEach((n) => n.classList.toggle("lit", on));
  wires.forEach((w) => w.classList.toggle("lit", on));
}

function clearPin() {
  if (!pinned) return;
  setLit(pinned.grid, pinned.line, false);
  pinned.grid.classList.remove("has-pin");
  pinned = null;
}

staves.forEach((grid) => {
  const figure = grid.querySelector(".figure");
  if (!figure) return;
  grid.querySelectorAll(".note[data-for]").forEach((note) => {
    const line = note.dataset.for;
    const row = figure.querySelector(`[data-line="${line}"]`);
    if (!row) return;
    const on = () => {
      if (pinned && (pinned.grid !== grid || pinned.line !== line)) return;
      setLit(grid, line, true);
    };
    const off = () => {
      if (pinned && pinned.grid === grid && pinned.line === line) return;
      setLit(grid, line, false);
    };
    const toggle = (e) => {
      e.stopPropagation();
      if (pinned && pinned.grid === grid && pinned.line === line) {
        clearPin();
      } else {
        clearPin();
        pinned = { grid, line };
        grid.classList.add("has-pin");
        setLit(grid, line, true);
      }
    };
    note.addEventListener("mouseenter", on);
    note.addEventListener("mouseleave", off);
    row.addEventListener("mouseenter", on);
    row.addEventListener("mouseleave", off);
    note.addEventListener("click", toggle);
    row.addEventListener("click", toggle);
  });
});

document.addEventListener("click", clearPin);
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") clearPin();
});

// ---- scrollspy ----

const railLinks = new Map(
  [...document.querySelectorAll('.rail a[href^="#"]')].map((a) => [
    a.getAttribute("href").slice(1),
    a,
  ])
);
let current = null;
const spy = new IntersectionObserver(
  (es) => {
    es.forEach((e) => {
      if (!e.isIntersecting) return;
      const link = railLinks.get(e.target.id);
      if (!link || link === current) return;
      current?.classList.remove("on");
      current?.removeAttribute("aria-current");
      link.classList.add("on");
      link.setAttribute("aria-current", "true");
      current = link;
      if (window.matchMedia("(max-width: 900px)").matches) {
        link.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "center" });
      }
    });
  },
  { rootMargin: "-45% 0px -50% 0px" }
);
document
  .querySelectorAll("#frontispiece, .chapter, .endmatter, #endpaper")
  .forEach((el) => spy.observe(el));
