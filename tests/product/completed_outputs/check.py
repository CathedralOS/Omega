#!/usr/bin/env python3
"""Check Linux completed-output publication through the public compiler CLI."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omega", type=Path, required=True)
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[3])
    options = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() not in ("x86_64", "amd64"):
        raise SystemExit("This qualification needs a Linux x86-64 native host.")
    root = options.repository.resolve()
    omega = options.omega.resolve()
    environment = {**os.environ, "OMEGA_LIBRARY_ROOT": str(root / "source/library")}
    source = root / "tests/omega/pass/build/acquired_generator_completed_output_set"
    with tempfile.TemporaryDirectory(prefix="omega-completed-outputs-") as temporary:
        workspace = Path(temporary)
        project = workspace / "project"
        shutil.copytree(source, project)
        output = workspace / "out"
        observations = []

        def invoke(*arguments):
            result = subprocess.run([str(omega), *map(str, arguments)], env=environment,
                                    stdin=subprocess.DEVNULL, capture_output=True, timeout=180)
            observations.append(dict(arguments=list(map(str, arguments)), exit=result.returncode,
                                     stdout=result.stdout.decode(errors="replace"),
                                     stderr=result.stderr.decode(errors="replace")))
            return result

        review = invoke("update", "--project", project, "--dangerously-accept-all")
        assert review.returncode == 0, review

        def compile_project():
            return invoke("--target", "linux_x86_64", "--build-dir", output, project / "main.omg")

        def successful_set(result):
            assert result.returncode == 0, result
            matches = re.findall(rb"^published completed build outputs to (.+)$",
                                 result.stdout, re.MULTILINE)
            assert len(matches) == 1, result
            directory = Path(os.fsdecode(matches[0]))
            assert directory.parent == output / "completed", directory
            assert len(list(directory.parent.iterdir())) == 1
            assert (directory / "files/companion.txt").read_bytes() == b"omega\n"
            assert (directory / "files/generated.omg").read_bytes() == b"public const GENERATED: u32 = 73;\n"
            assert (directory / "manifest.bin").read_bytes().startswith(b"OMEGA-COMPLETED-BUILD-OUTPUTS-V1\0")
            return directory

        directory = successful_set(compile_project())
        initial_manifest = (directory / "manifest.bin").read_bytes()
        assert successful_set(compile_project()) == directory, "valid repeat changed set identity"
        image = output / "omega-program"
        execution = subprocess.run([str(image)], stdin=subprocess.DEVNULL,
                                   capture_output=True, timeout=30)
        assert execution.returncode == 0 and execution.stdout == execution.stderr == b"", execution
        compared = invoke("run", "--both", project / "main.omg")
        assert compared.returncode == 0 and compared.stdout == b"", compared
        assert b"native exit 0\n" in compared.stderr and b"interp exit: 0\n" in compared.stderr, compared
        backup = workspace / "valid-set"
        shutil.copytree(directory, backup)
        external = workspace / "external.txt"
        external.write_bytes(b"omega\n")

        def restore():
            if directory.is_symlink():
                directory.unlink()
            else:
                shutil.rmtree(directory)
            shutil.copytree(backup, directory)

        def interrupted():
            shutil.rmtree(directory / "files")
            (directory / "manifest.bin").unlink()

        def member_link():
            (directory / "files/companion.txt").unlink()
            (directory / "files/companion.txt").symlink_to(external)

        def files_link():
            shutil.rmtree(directory / "files")
            (directory / "files").symlink_to(backup / "files", target_is_directory=True)

        def set_link():
            shutil.rmtree(directory)
            directory.symlink_to(backup, target_is_directory=True)

        def member_alias():
            (directory / "files/companion.txt").unlink()
            os.link(external, directory / "files/companion.txt")

        controls = [
            ("interrupted", interrupted),
            ("missing manifest", lambda: (directory / "manifest.bin").unlink()),
            ("forged manifest", lambda: (directory / "manifest.bin").write_bytes(b"forged\n")),
            ("missing member", lambda: (directory / "files/companion.txt").unlink()),
            ("substituted bytes", lambda: (directory / "files/companion.txt").write_bytes(b"wrong\n")),
            ("extra member", lambda: (directory / "files/extra.txt").write_bytes(b"uncommitted\n")),
            ("executable member", lambda: (directory / "files/companion.txt").chmod(0o755)),
            ("member symlink", member_link),
            ("files directory symlink", files_link),
            ("set directory symlink", set_link),
            ("hard-link alias", member_alias),
        ]
        for name, mutate in controls:
            mutate()
            result = compile_project()
            assert result.returncode == 1, (name, result)
            assert b"existing completed-output set" in result.stderr, (name, result)
            assert b"published completed build outputs" not in result.stdout, (name, result)
            assert list((output / "completed").iterdir()) == [directory], (name, result)
            assert external.read_bytes() == b"omega\n", name
            restore()
            assert successful_set(compile_project()) == directory, name
            assert (directory / "manifest.bin").read_bytes() == initial_manifest

        # Target children own separate result locations. ARM64 is generated
        # here, never presented as native execution on this x86-64 host.
        review = invoke("update", "--project", project, "--target", "linux_x86_64",
                        "--target", "linux_arm64", "--dangerously-accept-all")
        assert review.returncode == 0, review
        multi_output = workspace / "targets"
        multi = invoke("--target", "linux_x86_64", "--target", "linux_arm64",
                       "--build-dir", multi_output, project / "main.omg")
        assert multi.returncode == 0, multi
        target_sets = []
        for target in ("linux_x86_64", "linux_arm64"):
            sets = list((multi_output / target / "completed").iterdir())
            assert len(sets) == 1, (target, sets)
            assert (sets[0] / "files/companion.txt").read_bytes() == b"omega\n"
            assert (sets[0] / "files/generated.omg").read_bytes() == b"public const GENERATED: u32 = 73;\n"
            assert (sets[0] / "manifest.bin").is_file()
            target_sets.append(sets[0])
        assert target_sets[0] != target_sets[1]

        other = workspace / "other-package"
        shutil.copytree(source, other)
        declaration = other / "build.omg"
        declaration.write_text(declaration.read_text().replace(
            '"acquired_generator_completed_output_set"', '"another_output_package"'))
        review = invoke("update", "--project", other, "--dangerously-accept-all")
        assert review.returncode == 0, review
        other_output = workspace / "other-out"
        produced = invoke("--target", "linux_x86_64", "--build-dir", other_output, other / "main.omg")
        assert produced.returncode == 0, produced
        other_sets = list((other_output / "completed").iterdir())
        assert len(other_sets) == 1 and other_sets[0].name != directory.name, other_sets
        assert (other_sets[0] / "files/companion.txt").read_bytes() == b"omega\n"

        # A later generated-source check must fail before any set is exported.
        invalid = workspace / "invalid"
        fixture = root / "tests/omega/fail/build/acquired_generator_output_final_check_refused"
        shutil.copytree(fixture, invalid)
        invalid_output = workspace / "invalid-out"
        refused = invoke("--target", "linux_x86_64", "--build-dir", invalid_output, invalid / "main.omg")
        assert refused.returncode == 1 and b"scalar constant `GENERATED` is invalid" in refused.stderr, refused
        assert not (invalid_output / "completed").exists(), refused
        print(json.dumps(dict(host="linux_x86_64", native_exit=0, terminal_exit=0,
                              reuse_rejections=len(controls), successful_retries=len(controls),
                              final_check_rejection=True,
                              distinct_package_sets=2, target_exports=["linux_x86_64", "linux_arm64"],
                              manifest_sha256=hashlib.sha256(initial_manifest).hexdigest(),
                              observations=observations)))


if __name__ == "__main__":
    main()
