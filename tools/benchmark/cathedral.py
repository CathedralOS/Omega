"""Measure the explicit Cathedral composition; failed builds have no full-build metric."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import time

from benchmark import capture_compile, cpu_name, require_optimized


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_snapshot(snapshot, manifest):
    for name, record in manifest.items():
        if digest(snapshot / name) != record['sha256']:
            raise ValueError(f'frozen input changed: {name}')


def compiler_wall_sum_ms(entries, rows):
    expected = [entry['name'] for entry in entries]
    if not expected or [row['name'] for row in rows] != expected:
        return None
    if any(row.get('compile', {}).get('exit_code') != 0
           or not row.get('compile', {}).get('artifact_valid') for row in rows):
        return None
    return sum(row['compile']['wall_ms'] for row in rows)


def validate_artifacts(snapshot, entry, output):
    if entry['kind'] == 'uefi':
        image = output / 'omega-program.exe'
        contents = image.read_bytes()
        if len(contents) < 64 or contents[:2] != b'MZ':
            raise ValueError('missing PE image header')
        offset = int.from_bytes(contents[60:64], 'little')
        if contents[offset:offset + 4] != b'PE\0\0':
            raise ValueError('missing PE signature')
        return [image]
    module_path = snapshot / 'tools/omega-component/elf.py'
    specification = importlib.util.spec_from_file_location('cathedral_elf', module_path)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    image = output / 'omega-program'
    dependencies = module.Elf(image.read_bytes(), 2).needed()
    dependency = dependencies[0].decode() if len(dependencies) == 1 else ''
    filename = dependency.removeprefix('$ORIGIN/')
    if not filename or Path(filename).name != filename or dependency != '$ORIGIN/' + filename:
        raise ValueError('expected exactly one local native component')
    component = output / filename
    module.component(component.read_bytes())
    description = component.with_suffix('.so.component')
    if not description.is_file() or not description.stat().st_size:
        raise ValueError('missing native component description')
    return [image, component, description]


def compile_entry(compiler, snapshot, entry, directory):
    output = directory / 'product'
    if output.exists():
        raise ValueError(f'refusing existing product directory: {output}')
    command = [str(compiler), '--offline', '--timings', '--target', entry['target'],
               '--build-dir', str(output), str(snapshot / entry['source'])]
    code, elapsed, peak, text = capture_compile(command, str(snapshot))
    (directory / 'compile.log').write_text(text, encoding='utf-8')
    record = dict(command=command, exit_code=code, wall_ms=elapsed,
                  peak_bytes=peak, artifact_valid=False)
    # A failed compiler may leave files; none qualify as a produced artifact.
    if code == 0:
        try:
            paths = validate_artifacts(snapshot, entry, output)
            record['artifacts'] = {str(path.relative_to(output)): dict(
                sha256=digest(path), bytes=path.stat().st_size) for path in paths}
            record['artifact_valid'] = True
        except (OSError, ValueError) as error:
            record['artifact_error'] = str(error)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, required=True)
    parser.add_argument('--snapshot', type=Path, required=True)
    parser.add_argument('--source-manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    options = parser.parse_args()
    snapshot = options.snapshot.resolve()
    compiler = options.compiler.resolve()
    directory = options.output.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    manifest = json.loads(options.source_manifest.read_text(encoding='utf-8'))
    composition_path = Path(__file__).with_suffix('.json')
    composition = json.loads(composition_path.read_text(encoding='utf-8'))
    verify_snapshot(snapshot, manifest)
    profile = require_optimized(compiler)
    os.environ['OMEGA_PHASE_PROFILE'] = '0'
    report = dict(composition=composition, composition_sha256=digest(composition_path),
                  source_manifest_sha256=digest(options.source_manifest), compiler=profile,
                  recorded_utc=time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
                  host=dict(os=platform.platform(), cpu=cpu_name(), logical_cpus=os.cpu_count()),
                  cache_state='fresh per-entry output; persistent dependency/source caches not cleared',
                  preparation_included_in_compile_metric=False, runtime_validated=False,
                  complete_compile_ms=None, entries=[])
    def save():
        (directory / 'result.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    save()
    for entry in composition['entries']:
        working = directory / entry['name']
        working.mkdir()
        source = snapshot / entry['source']
        if not source.is_file():
            raise ValueError(f'missing composition source: {source}')
        command = [str(compiler), 'update', '--project', str(source.parent),
                   '--target', entry['target'], '--offline']
        print(f"{entry['name']}: prepare", flush=True)
        start = time.monotonic()
        with (working / 'prepare.log').open('wb') as log:
            prepared = subprocess.run(command, cwd=snapshot, stdin=subprocess.DEVNULL,
                                      stdout=log, stderr=subprocess.STDOUT)
        row = dict(name=entry['name'], preparation=dict(command=command,
                   exit_code=prepared.returncode, wall_ms=(time.monotonic() - start) * 1000))
        if prepared.returncode != 0:
            row['compile_not_attempted'] = 'ordinary package preparation failed; inspect prepare.log'
        report['entries'].append(row)
        save()
        print(f"{entry['name']}: preparation exit {prepared.returncode}", flush=True)
    compile_start = time.monotonic()
    for entry, row in zip(composition['entries'], report['entries']):
        if row['preparation']['exit_code'] != 0:
            continue
        print(f"{entry['name']}: compile", flush=True)
        row['compile'] = compile_entry(compiler, snapshot, entry, directory / entry['name'])
        save()
        print(f"{entry['name']}: artifact valid {row['compile']['artifact_valid']}", flush=True)
    compile_wall_ms = (time.monotonic() - compile_start) * 1000
    verify_snapshot(snapshot, manifest)
    if digest(compiler) != profile['sha256']:
        raise ValueError('compiler changed during measurement')
    report['compiler_wall_sum_ms'] = compiler_wall_sum_ms(composition['entries'], report['entries'])
    if report['compiler_wall_sum_ms'] is not None:
        report['complete_compile_ms'] = compile_wall_ms
    report['inputs_unchanged'] = True
    save()
    print(json.dumps(dict(complete_compile_ms=report['complete_compile_ms'],
                         runtime_validated=False, result=str(directory / 'result.json'))))
    return 0 if report['complete_compile_ms'] is not None else 1


if __name__ == '__main__':
    raise SystemExit(main())
