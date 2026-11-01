"""Seed each signal's known answer, rather than only running it on today's tree."""

from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import architecture_signals as signals
from architecture_signals import (
    clone_pairs, co_change, crate_map, decisions, erased_tokens, instrument, module_clone_mass,
    module_key, parse_history, parse_trace, pipeline_route, repair_hot_spots, route_chains,
    route_table, same_name_families, shingles,
)

CRATES = {
    "omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees": "psi:04",
    "omega-rust/psi/compilation/checking": "validation",
}
STAGE_04 = "omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src"
CHECKING = "omega-rust/psi/compilation/checking/src"


def log(*commits):
    lines = []
    for number, (date, subject, paths) in enumerate(commits):
        lines.append(f"@@{number:040x}\t{date}T00:00:00+00:00\t{subject}")
        lines.extend(paths)
        lines.append("")
    return "\n".join(lines)


class ModuleKeyTests(unittest.TestCase):
    def test_rust_paths_name_crate_and_top_module(self):
        self.assertEqual(module_key(f"{STAGE_04}/checks/contracts/exits.rs", CRATES),
                         "psi:04::checks")
        self.assertEqual(module_key(f"{CHECKING}/proof_contracts.rs", CRATES),
                         "validation::proof_contracts")

    def test_a_moved_crate_keeps_its_directory(self):
        self.assertEqual(module_key("omega-rust/old/place/src/a/b.rs", CRATES),
                         "omega-rust/old/place::a")

    def test_product_sources_and_non_code(self):
        self.assertEqual(module_key("source/psi/checking/arrivals.omg", CRATES),
                         "src:psi/checking::arrivals")
        self.assertIsNone(module_key("tests/omega/pass/a/main.omg", CRATES))
        self.assertIsNone(module_key("omega-rust/pipeline.md", CRATES))


class HistoryTests(unittest.TestCase):
    def commits(self):
        both = [f"{STAGE_04}/checks/a.rs", f"{CHECKING}/proof_contracts/b.rs"]
        mass = [f"omega-rust/x{index}/src/m.rs" for index in range(20)]
        return parse_history(log(
            ("2026-10-20", "psi: preserve saved values across calls", both),
            ("2026-10-21", "psi: retain loan facts through joins", both),
            ("2026-10-22", "psi: delete exit routes that never decide", both),
            ("2026-10-23", "psi: keyword-free rename", [f"{STAGE_04}/checks/a.rs"]),
            ("2026-10-24", "repo: migrate every crate", mass + both),
        ), CRATES)

    def test_co_change_counts_cross_crate_pairs_and_skips_mass_migrations(self):
        pairs, singles, skipped = co_change(self.commits(), max_modules=15)
        self.assertEqual(skipped, 1)
        self.assertEqual(pairs[("psi:04::checks", "validation::proof_contracts")], 3)
        self.assertEqual(singles["psi:04::checks"], 4)

    def test_same_crate_modules_are_not_a_pair(self):
        commits = parse_history(log(("2026-10-20", "psi: x", [
            f"{STAGE_04}/checks/a.rs", f"{STAGE_04}/flow/b.rs"])), CRATES)
        pairs, _, _ = co_change(commits, 15)
        self.assertEqual(pairs, {})

    def test_repair_verbs_anywhere_in_the_subject(self):
        rows = {row[4]: row for row in repair_hot_spots(self.commits())}
        repairs, total, _, recent, _ = rows["psi:04::checks"]
        # preserve, retain, plus the mass migration's subject has no verb;
        # "keyword" is not "keep".
        self.assertEqual((repairs, total, recent), (2, 5, 2))

    def test_every_listed_verb_counts(self):
        for verb in signals.REPAIR_VERBS:
            self.assertTrue(signals.REPAIR.search(f"psi: {verb}s facts"), verb)
        self.assertFalse(signals.REPAIR.search("psi: delete duplicate routes"))


class CloneTests(unittest.TestCase):
    BODY = """
        fn {name}(program: &Program, fact: &Fact) -> bool {{
            let {local} = program.custody_of(fact.value);
            match {local}.claim {{
                Claim::Borrowed {{ origin, .. }} => origin.retires_before(fact.point, 3),
                Claim::Owned => program.release_root({local}.root, "owned"),
                _ => false,
            }}
        }}
    """

    def files(self):
        first = erased_tokens(self.BODY.format(name="a", local="custody") * 3)
        second = erased_tokens(self.BODY.format(name="b", local="held") * 3)
        unrelated = erased_tokens("struct Grid { width: u16, cells: [u8; 64] }\n"
                                   "impl Grid { const SIZE: usize = 64; }" * 3)
        return {
            ("psi:04::checks", "checks/custody.rs"): shingles(first, sample=1),
            ("validation::value_custody", "value_custody/custody.rs"): shingles(second, sample=1),
            ("psi:04::flow", "flow/custody.rs"): shingles(second, sample=1),
            ("omega:03::other", "other/math.rs"): shingles(unrelated, sample=1),
        }

    def test_locals_and_literals_are_erased_names_kept(self):
        tokens = erased_tokens('let held = program.custody_of(fact, "x", 3);')
        self.assertEqual(tokens, ["let", "x", "=", "x", ".", "custody_of", "(", "x", ",", "s",
                                  ",", "0", ")", ";"])

    def test_renamed_reimplementations_pair_across_crates_only(self):
        rows = clone_pairs(self.files(), max_sharing=4)
        pairs = {(left[0], right[0]) for _, _, left, right in rows}
        self.assertIn(("psi:04::checks", "validation::value_custody"), pairs)
        self.assertIn(("psi:04::flow", "validation::value_custody"), pairs)
        self.assertNotIn(("psi:04::checks", "psi:04::flow"), pairs)
        self.assertFalse(any("omega:03::other" in pair for pair in pairs))
        self.assertGreater(rows[0][1], 0.9)

    def test_mass_and_same_name_families(self):
        rows = clone_pairs(self.files(), max_sharing=4)
        mass = module_clone_mass(rows)
        self.assertEqual({row[2:] for row in mass},
                         {("psi:04::checks", "validation::value_custody"),
                          ("psi:04::flow", "validation::value_custody")})
        families = same_name_families(rows)
        self.assertEqual(families[0][1], "custody")
        self.assertEqual(len(families[0][2]), 3)

    def test_shingles_shared_by_many_files_carry_no_signal(self):
        files = self.files()
        for index in range(5):
            files[(f"crate{index}::m", "m.rs")] = files[("psi:04::checks", "checks/custody.rs")]
        pairs = {(left[0], right[0]) for _, _, left, right in clone_pairs(files, max_sharing=4)}
        self.assertNotIn(("psi:04::checks", "validation::value_custody"), pairs)


PIPELINE_MD = """
## Connected program route

| Input → output | Owner |
| --- | --- |
| Typed → checked trees | [typed-trees-to-checked-trees](psi/compilation/pipeline/04_typed-trees-to-checked-trees/README.md) |
| Checked trees → lowered Psi | [checked-trees-to-lowered-psi](psi/compilation/pipeline/05_checked-trees-to-lowered-psi/README.md) |
| Resolved program → machine bytes | [machine-emission](omega/compilation/backend/machine-emission/README.md) |

## Next section

| Ignored | [x-to-y](z) |
"""


def package(root, name, directory, dependencies=()):
    return {"name": name, "manifest_path": f"{root}/{directory}/Cargo.toml",
            "dependencies": [{"name": dependency, "path": f"{root}/x", "kind": None}
                             for dependency in dependencies]}


class CrateMapTests(unittest.TestCase):
    def mapping(self):
        root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, root)
        stage = "omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi"
        (root / stage / "src").mkdir(parents=True)
        (root / stage / "src" / "lib.rs").write_text(
            "#![forbid(unsafe_code)]\n\n//! Checked trees to Lowered Psi. More.\n//!\n//! Body.\n")
        metadata = {"packages": [
            package(root, "typed-trees-to-checked-trees",
                    "omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees",
                    ["typed-trees", "checked-trees"]),
            package(root, "checked-trees-to-lowered-psi", stage,
                    ["checked-trees", "lowered-psi", "typed-trees",
                     "typed-trees-to-checked-trees"]),
            package(root, "lowered-psi-to-terminal-psi",
                    "omega-rust/psi/compilation/pipeline/07_lowered-psi-to-terminal-psi",
                    ["lowered-psi", "terminal-psi"]),
            package(root, "typed-trees", "omega-rust/psi/compilation/representations/typed-trees"),
            package(root, "checked-trees",
                    "omega-rust/psi/compilation/representations/checked-trees"),
            package(root, "lowered-psi", "omega-rust/psi/compilation/representations/lowered-psi"),
            package(root, "terminal-psi",
                    "omega-rust/psi/compilation/representations/terminal-psi"),
            package(root, "layout", "omega-rust/omega/compilation/backend/layout",
                    ["typed-trees", "terminal-psi"]),
            package(root, "image-emission", "omega-rust/omega/compilation/backend/images/x",
                    ["terminal-installation"]),
            package(root, "terminal-installation",
                    "omega-rust/psi/compilation/checking/terminal-installation"),
        ]}
        return crate_map(metadata, PIPELINE_MD, root)

    def test_route_table_is_read_from_its_section_only(self):
        self.assertEqual([owner for _, owner, _ in pipeline_route(PIPELINE_MD)],
                         ["typed-trees-to-checked-trees", "checked-trees-to-lowered-psi",
                          "machine-emission"])

    def test_stage_missing_from_pipeline_md(self):
        mapping = self.mapping()
        self.assertEqual(mapping["missing_from_pipeline_md"], ["lowered-psi-to-terminal-psi"])
        self.assertEqual(mapping["missing_from_workspace"], [])

    def test_stage_reading_another_representation_and_calling_a_stage(self):
        stage = {row["crate"]: row for row in self.mapping()["stages"]}[
            "checked-trees-to-lowered-psi"]
        self.assertEqual((stage["label"], stage["input"], stage["output"]),
                         ("psi:05", "checked-trees", "lowered-psi"))
        self.assertEqual(stage["other_representations"], ["typed-trees"])
        self.assertEqual(stage["pipeline_dependencies"], ["typed-trees-to-checked-trees"])
        self.assertEqual(stage["header"], "Checked trees to Lowered Psi.")

    def test_omega_reading_source_side_psi_is_a_leak(self):
        self.assertEqual(self.mapping()["firewall"], [("layout", "typed-trees")])


ROUTES = """
fn helper() {}

fn decide(values: &[u32], value: u32, proved_by_other: bool) -> bool {
    let alive = |x: u32| x > 0 || x == 7;
    debug_assert!(alive(1) || alive(2));
    let names = value > 0 || value == 0;
    let proved = (dead(value) || needed(value) || inner(value)) || redundant(value);
    if !proved && names && let Some(_) = fallback(value) {
        return true;
    }
    proved || proved_by_other
}

fn inner(value: u32) -> bool {
    also_needed(value) || shadowed(value)
}
"""


class InstrumentationTests(unittest.TestCase):
    def test_route_chains_skip_closures_macros_and_comparisons(self):
        chains = route_chains(ROUTES, "decide")
        labels = [[signals.operand_label(ROUTES[start:end]) for start, end in chain.operands]
                  for chain in chains]
        self.assertEqual(labels, [["dead", "redundant"], ["dead", "needed", "inner"]])

    def test_fallback_route_is_found(self):
        chains = route_chains(ROUTES, "decide")
        ((_, label, condition),) = signals.fallback_routes(ROUTES, "decide", chains)
        self.assertEqual(label, "fallback")
        self.assertEqual(condition, "(names) && (fallback(value).is_some())")

    def test_instrumented_routes_report_dead_redundant_and_needed(self):
        rustc = shutil.which("rustc")
        if rustc is None:
            self.skipTest("rustc is not installed")
        text, chains, fallbacks = instrument(ROUTES, "decide", "decide")
        text, inner, _ = instrument(text, "inner", "inner")
        self.assertEqual((chains, fallbacks, inner), (2, 1, 1))
        program = text + """
fn dead(_: u32) -> bool { false }
fn needed(value: u32) -> bool { value == 1 }
fn redundant(value: u32) -> bool { value == 1 || value == 2 }
fn also_needed(value: u32) -> bool { value == 2 || value == 4 }
fn shadowed(value: u32) -> bool { value == 2 }
fn fallback(value: u32) -> Option<()> { (value == 3).then_some(()) }
fn main() {
    helper();
    for value in [1, 2, 3, 4, 5] {
        decide(&[], value, false);
    }
}
""" + signals.TRACE_MODULE
        directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, directory)
        (directory / "main.rs").write_text(program)
        subprocess.run([rustc, "--edition", "2024", "-A", "warnings", "-o",
                        str(directory / "routes"), str(directory / "main.rs")], check=True,
                       capture_output=True)
        run = subprocess.run([str(directory / "routes")], capture_output=True, text=True,
                             check=True)
        records = parse_trace(run.stderr)
        rows = {row["route"]: row for row in route_table({"seed": records})
                if row["site"] == "decide"}
        # value 1: needed + redundant; 2: also_needed + shadowed + redundant;
        # 3: fallback alone; 4: also_needed alone (through the nested chain);
        # 5: nothing.
        self.assertEqual(rows["dead"]["verdict"], "dead")
        self.assertEqual(rows["redundant"]["verdict"], "redundant")
        self.assertEqual(rows["shadowed"]["verdict"], "redundant")
        self.assertEqual((rows["also_needed"]["verdict"], rows["also_needed"]["sole"]),
                         ("needed", 1))
        self.assertEqual(rows["needed"]["verdict"], "redundant")
        self.assertEqual((rows["fallback"]["verdict"], rows["fallback"]["sole"]), ("needed", 1))
        self.assertEqual(rows["dead"]["evaluated"], 5)
        self.assertNotIn("inner", rows)


def record(thread, sequence, parent, operand, site, labels, bits):
    return "\t".join(["OMEGA_ROUTE_TRACE", thread, str(sequence), str(parent), str(operand),
                      site, "0", "|".join(labels), bits])


class AnalysisTests(unittest.TestCase):
    def test_nested_chain_replaces_its_operand(self):
        text = "\n".join([
            "unrelated stderr line",
            record("T", 2, 1, 0, "outer", ["a", "b"], "01"),
            record("T", 1, 0, 0, "outer", ["guarded", "c"], "10"),
        ])
        ((root, routes),) = decisions(parse_trace(text))
        self.assertEqual(routes, {"a": False, "b": True, "c": False})

    def test_a_guard_that_failed_keeps_the_operand(self):
        text = "\n".join([
            record("T", 2, 1, 0, "outer", ["a", "b"], "01"),
            record("T", 1, 0, 0, "outer", ["guarded", "c"], "01"),
        ])
        ((_, routes),) = decisions(parse_trace(text))
        self.assertEqual(routes, {"guarded": False, "c": True})

    def test_threads_do_not_mix(self):
        text = "\n".join([
            record("T1", 2, 1, 0, "outer", ["a", "b"], "10"),
            record("T2", 1, 0, 0, "outer", ["guarded", "c"], "10"),
            record("T1", 1, 0, 0, "outer", ["guarded", "c"], "10"),
        ])
        routes = sorted(sorted(entry[1].items()) for entry in decisions(parse_trace(text)))
        self.assertEqual(routes, [[("a", True), ("b", False), ("c", False)],
                                  [("c", False), ("guarded", True)]])

    def test_witness_limit(self):
        directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, directory)
        witnesses = directory / "w.txt"
        witnesses.write_text("# header\n" + "\n".join(f"pass/a/{n}" for n in range(201)))
        self.assertEqual(len(signals.read_witnesses(witnesses)), 201)
        with self.assertRaises(SystemExit):
            signals.route_coverage(type("A", (), {"witnesses": str(witnesses)})())


if __name__ == "__main__":
    unittest.main()
