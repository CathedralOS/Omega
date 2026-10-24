// verify.ts — checks the book's two promises:
//   1. Every "code" figure compiles under `omega --check` (fragments that quote
//      larger sources may set `verify: false` and are skipped).
//   2. Every keyed note (`for: N`) points at a real, non-blank line.
//
// Run: bun tools/verify.ts
// Requires the reference compiler at OMEGA_EXE (default: the sibling repo's
// debug build) and the bundled std at OMEGA_STD for figures that `use` it.
// OMEGA_LIBRARY_ROOT pins the compiler's bundled `omega::` packages; it
// defaults to the parent of OMEGA_STD.
//
// Fixture dirs persist under scratch/verify/ so each figure's `omega.lock`
// survives between runs. Since 2026-10, binding an entry slot whose schema
// comes from a dependency needs an accepted program-entry binding: the first
// `omega update` writes a review file, pending decision rows are flipped to
// `accept`, and `omega update --resume` publishes the lock. That ceremony
// re-runs only when a check reports missing or stale acceptance.
//
// Compiles run in parallel (cap 4): a cold fixture that `depend`s on std takes
// minutes because each fresh package location re-checks the whole dependency
// closure, so serial is the slow way to pay that cost.

import { chapters } from "../src/chapters";
import { mkdirSync, writeFileSync, existsSync, rmSync, readFileSync } from "fs";
import { join } from "path";

const OMEGA = process.env.OMEGA_EXE ?? "C:/SoftwareDevelopmentKits/Omega/target/debug/omega.exe";
const STD = process.env.OMEGA_STD ?? "C:/SoftwareDevelopmentKits/Omega/source/library/std";
if (!process.env.OMEGA_LIBRARY_ROOT) process.env.OMEGA_LIBRARY_ROOT = join(STD, "..");
const JOBS = 4;
const FIXTURES = join(import.meta.dir, "..", "scratch", "verify");

let failures = 0;
let keyed = 0;

const fail = (msg: string) => { failures++; console.error(`  ✗ ${msg}`); };

const run = async (args: string[], cwd: string) => {
  const proc = Bun.spawn(args, { cwd, stdout: "pipe", stderr: "pipe" });
  const out = (await proc.stdout.text()) + (await proc.stderr.text());
  return { code: await proc.exited, out };
};

const check = (dir: string) => run([OMEGA, "--check", join(dir, "main.omg")], dir);

// `omega update` writes review-<target>.txt; pending rows flip to `accept`,
// then `--resume` publishes omega.lock. Mirrors the sample gate's accept_review.
const acceptReview = async (dir: string, target: string) => {
  await run([OMEGA, "update", "--project", dir, "--target", target], dir);
  const review = join(dir, "build", "package-manager", `review-${target}.txt`);
  if (!existsSync(review)) return;
  const text = readFileSync(review, "utf8");
  if (!/^decision row [0-9a-f]+ pending$/m.test(text)) return;
  writeFileSync(review, text.replace(/^(decision row [0-9a-f]+) pending$/gm, "$1 accept"));
  await run([OMEGA, "update", "--project", dir, "--resume"], dir);
};

const ACCEPTANCE =
  /accepted program-entry binding|source identity changed|immutable content differs|unsupported package lock|review is pending|accepted lock|run `?omega update`?/i;

// --check against the fixture dir, completing the package review for each
// bound target when the compiler asks for it.
const checkAccepted = async (dir: string, buildCode: string | null) => {
  let r = await check(dir);
  for (let attempt = 0; attempt < 2 && r.code !== 0 && ACCEPTANCE.test(r.out); attempt++) {
    if (/immutable content differs|unsupported package lock/i.test(r.out)) {
      rmSync(join(dir, "omega.lock"), { force: true });
      rmSync(join(dir, "build"), { recursive: true, force: true });
    }
    const targets = buildCode
      ? [...buildCode.matchAll(/Build::bind\(\s*builder\s*,\s*(\w+)::ProgramEntry/g)].map((m) => m[1])
      : [];
    for (const target of targets.length ? targets : ["windows_x86_64"]) {
      await acceptReview(dir, target);
    }
    r = await check(dir);
  }
  return r;
};

type Job = { where: string; dir: string; buildCode: string | null };
const jobs: Job[] = [];
const skipped: string[] = [];

for (const ch of chapters) {
  ch.staves.forEach((staff, si) => {
    const where = `${ch.id} staff ${si + 1}${staff.title ? ` "${staff.title}"` : ""}`;
    const fig = staff.figure;
    if (fig.kind !== "code") return;

    const lines = fig.code.split("\n");
    for (const n of staff.notes) {
      if (n.for == null) continue;
      keyed++;
      if (n.for < 1 || n.for > lines.length) {
        fail(`${where}: note key ${n.for} out of range (block has ${lines.length} lines)`);
      } else if (lines[n.for - 1].trim() === "") {
        fail(`${where}: note key ${n.for} points at a blank line`);
      }
    }
    for (const h of fig.highlight ?? []) {
      if (h < 1 || h > lines.length || lines[h - 1].trim() === "")
        fail(`${where}: highlight ${h} is out of range or blank`);
    }

    if ((fig as { verify?: boolean }).verify === false) {
      skipped.push(where);
      return;
    }
    const f = fig as { filename?: string; companion?: string };
    const dir = join(FIXTURES, `${ch.id}-s${si + 1}`);
    mkdirSync(dir, { recursive: true });
    // build.omg excerpts display a relative std location like the real
    // samples; verification substitutes the actual std path.
    const code = f.filename === "build.omg"
      ? fig.code.replace(/location:\s*"[^"]*"/, `location: ${JSON.stringify(STD).replace(/\\/g, "/")}`)
      : fig.code;
    writeFileSync(join(dir, f.filename ?? "main.omg"), code + "\n");
    if (f.companion) writeFileSync(join(dir, "main.omg"), f.companion + "\n");
    const needsStd = /omega_language_std|omega::/.test(fig.code) || f.companion != null;
    const buildPath = join(dir, "build.omg");
    let buildCode: string | null = null;
    if (f.filename === "build.omg") {
      buildCode = code;
    } else if (needsStd) {
      const entry = /\bmachine start\b/.test(fig.code) ? "start" : "main";
      buildCode =
        `machine build(builder: &mut Build) {\n` +
        `    Build::application(builder, "omegalearn_fixture");\n` +
        `    Build::depend(builder, Source::Path { location: ${JSON.stringify(STD).replace(/\\/g, "/")} });\n` +
        `    Build::bind(builder, windows_x86_64::ProgramEntry, ${entry});\n` +
        `}\n`;
      writeFileSync(buildPath, buildCode);
    } else if (existsSync(buildPath)) {
      rmSync(buildPath);
    }
    jobs.push({ where, dir, buildCode });
  });
}

for (const w of skipped) console.log(`  - ${w}: fragment (verify:false), skipped compile`);

type Result = { where: string; code: number; out: string };
const flat: Result[] = [];
// batches of JOBS: one omega.exe per fixture dir, four lanes at a time
for (let i = 0; i < jobs.length; i += JOBS) {
  flat.push(...await Promise.all(jobs.slice(i, i + JOBS).map(async ({ where, dir, buildCode }) => {
    const r = await checkAccepted(dir, buildCode);
    return { where, code: r.code, out: r.out };
  })));
}

// report in document order
for (const { where } of jobs) {
  const r = flat.find((x) => x.where === where)!;
  if (r.code !== 0 || /error:/i.test(r.out)) {
    fail(`${where}: omega --check failed\n${r.out.trim().split("\n").map((l) => "      " + l).join("\n")}`);
  } else {
    console.log(`  ✓ ${where}: compiles`);
  }
}

console.log(`\n${jobs.length} code blocks compile-checked; ${keyed} note keys validated; ${failures} failure(s)`);
process.exit(failures ? 1 : 0);
