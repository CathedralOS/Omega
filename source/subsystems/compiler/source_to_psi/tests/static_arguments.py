"""Run the Omega-written call binder and resolver comparisons with small input tables."""

import argparse
import re
import shutil
import subprocess
import tempfile
from pathlib import Path


PACKAGE = Path(__file__).resolve().parents[1]
RESOLVER = "pipeline/syntax_trees_to_symbol_resolved_trees/"
TYPER = "pipeline/symbol_resolved_trees_to_typed_trees/"
DECLARATIONS = {
    "representations/syntax_trees/syntax_trees.omg": [
        "GenericParameterKind", "TypeReferenceKind", "ReferenceAccess", "IntegerLiteral",
        "static_integer_is_valid", "same_type_form",
    ],
    "representations/symbol_resolved_trees/symbol_resolved_trees.omg": [
        "BuiltinType", "TypeBinding", "MachineArgument", "DomainBinding", "TypeScope",
        "name_resolution_machine_argument",
    ],
    RESOLVER + "generic_scopes.omg": ["GenericCoordinate", "generic_coordinate", "same_generic_coordinate", "generic_annotation_role"],
    RESOLVER + "machine_signatures.omg": ["same_type_row"],
    RESOLVER + "domains.omg": ["same_head", "same_bound_rows"],
    RESOLVER + "type_names.omg": ["TypeRole", "trait_name_admitted"],
    TYPER + "references.omg": ["slot_accepts", "static_parameter_annotation", "trait_slot_accepts", "plain_static_name"],
    TYPER + "calls.omg": ["StaticArguments", "bind_static_argument", "bind_static_arguments"],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("omega", type=Path, help="path to an optimized omega executable")
    args = parser.parse_args()
    fixture = PACKAGE / "tests/fixtures/generics/run/static_arguments"
    units = []
    for filename, names in DECLARATIONS.items():
        source = (PACKAGE / filename).read_text()
        for name in names:
            # A top-level declaration closes at the left margin. Nested machines
            # stay with their owner, exactly as compiled in the product package.
            match = re.search(rf"^(?:public )?(?:data|machine) {name}\b.*?^}}", source, re.M | re.S)
            if not match:
                raise ValueError(f"missing declaration {name} in {filename}")
            units.append(match[0])

    with tempfile.TemporaryDirectory(prefix="omega-static-arguments-") as directory:
        root = Path(directory)
        (root / "build.omg").write_bytes((fixture / "build.omg").read_bytes())
        numeric = Path("representations/tokens/numeric.omg")
        (root / numeric).parent.mkdir(parents=True)
        shutil.copyfile(PACKAGE / numeric, root / numeric)
        for state in (Path(TYPER) / "typing_state.omg", Path(RESOLVER) / "resolution_state.omg"):
            (root / state.parent).mkdir(parents=True)
            shutil.copyfile(fixture / state.name, root / state)
            units.insert(0, f"use {state.as_posix()};")
        program = root / "main.omg"
        units.insert(0, "use representations/tokens/numeric.omg;")
        units.append((fixture / "main.omg").read_text())
        program.write_text("\n\n".join(units))
        result = subprocess.run(
            [args.omega.resolve(), "run", "--both", root / "build.omg"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        print(result.stdout, end="")
        expected = {"native exit 0", "interp exit: 0"}
        if result.returncode or not expected.issubset(result.stdout.splitlines()):
            raise SystemExit(1)


if __name__ == "__main__":
    main()
