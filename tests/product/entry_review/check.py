#!/usr/bin/env python3
"""Exercise package entry nomination through the public update command.

No review is accepted. The positive case must produce a pending review; two
negative controls must reject before offering one. Temporary packages reuse
the checked-in calling policy rather than maintaining a second ABI fixture.
"""

import argparse
import os
from pathlib import Path
import subprocess
import tempfile


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")


def dependency(alias, path):
    return f'Build::depend_as(builder, "{alias}", Source::Path {{ location: "{path.as_posix()}" }});'


def check(omega, repository, workspace, mode):
    root = workspace / mode
    selected, alternative, application = (root / name for name in ("selected", "alternative", "app"))
    standard = repository / "source/library/std"
    contract = (standard / "targets/uefi_x86_64/entry.omg").read_text(encoding="utf-8")
    contract = contract.replace("use calling;", "use std::calling;")
    contract = contract.replace("pub boundary data EfiSystemTable;", "pub data EfiSystemTable { signature: u64; }")
    contract = contract.replace("\nUefiX86_64CallingPolicy satisfies", "\npub UefiX86_64CallingPolicy satisfies")
    for name, package in (("selected", selected), ("alternative", alternative)):
        extra = dependency("selected", selected) if name == "alternative" and mode == "shared_requirement" else ""
        write(package / "build.omg", f'''machine build(builder: &mut Build) {{
    Build::package(builder, "entry_{name}");
    {dependency("std", standard)}
    {extra}
}}
''')
        write(package / "main.omg", "use entry;\n")
        body = contract
        if name == "alternative" and mode == "shared_requirement":
            body = '''use omega::language::core::extent;
use std::calling;
use selected::entry;
boundary trait UefiApplication: ProgramStorageEntry + selected::UefiPhysicalEntry + Calling<selected::UefiX86_64CallingPolicy> { }
'''
        write(package / "entry.omg", body)
    selection = "" if mode == "no_selection" else "Build::select_provider<selected::UefiPhysicalEntry, Adapter>(builder);"
    write(application / "build.omg", f'''machine build(builder: &mut Build) {{
    Build::application(builder, "entry_review");
    {dependency("std", standard)}
    {dependency("selected", selected)}
    {dependency("alternative", alternative)}
    {selection}
    Build::bind(builder, uefi_x86_64::ProgramEntry, kernel);
    builder.subsystem = Subsystem::EfiApplication;
    builder.freestanding = true;
}}
''')
    source = '''use omega::language::core::extent;
use selected::entry;
Adapter satisfies UefiPhysicalEntry {
    UefiPhysicalEntry::enter = physical;
}
machine physical(handle: EfiImageHandle, table: &EfiSystemTable) -> EfiStatus
satisfies UefiPhysicalEntry::enter {
    EfiStatus { code: 0x8000000000000002 }
}
machine kernel(image: Extent in Granted, storage: Extent in Granted) {
    transition { _ -> retain(image, storage) }
    state retain(image: Extent in Granted, storage: Extent in Granted) {
        transition { _ -> retain(image, storage) }
    }
}
'''
    if mode == "shared_requirement":
        source = "use alternative::entry;\n" + source
    if mode == "no_selection":
        source = source.replace('''Adapter satisfies UefiPhysicalEntry {
    UefiPhysicalEntry::enter = physical;
}
''', "")
        source = source.replace("satisfies UefiPhysicalEntry::enter {", "{")
    write(application / "main.omg", source)
    result = subprocess.run(
        [str(omega), "update", "--project", str(application), "--target", "uefi_x86_64"],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
        env={**os.environ, "OMEGA_LIBRARY_ROOT": str(repository / "source/library")},
    )
    output = result.stdout + result.stderr
    review = application / "build/package-manager/review-uefi_x86_64.txt"
    if mode == "selected":
        assert review.exists() and "pending" in review.read_text(encoding="utf-8"), output[-12000:]
        assert not (application / "omega.lock").exists(), "nomination granted acceptance"
    else:
        # The shared-requirement source imports two same-named schemas. Psi
        # may reject that collision before entry selection; neither outcome
        # permits silently picking the first schema.
        expected = ("multiple package-owned", "duplicate trait `UefiApplication`") if mode == "shared_requirement" else ("accepted",)
        assert result.returncode != 0 and not review.exists() and any(text in output for text in expected), output[-12000:]
    print(f"entry-review {mode}: passed", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omega", type=Path, required=True)
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--case", choices=("selected", "no_selection", "shared_requirement"))
    options = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="omega-entry-review-") as directory:
        for mode in ([options.case] if options.case else ("selected", "no_selection", "shared_requirement")):
            check(options.omega.resolve(), options.repository.resolve(), Path(directory), mode)


if __name__ == "__main__":
    main()
