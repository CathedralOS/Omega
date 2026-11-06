"""Relocate one authenticated retained baseline producer; never build it."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import sys
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
from ci import compiler_replay_plan
import corpus_gate

ARTIFACT = 13487194906
ARTIFACT_NAME = 'compiler-pilot-linux_x86_64-a2855f01e81002f24233399afb13b98849b45d00-1'
ARTIFACT_DIGEST = '4b15c35125d095fca8a068c12b1ccb02806bc8f2393f858d526635081e5a470c'
ARTIFACT_BYTES = 481357243
REPOSITORY = 'CathedralOS/Omega'
PLAN = '@tools/ci/scalar_handback_successor_baseline.json'


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def main():
    plan = compiler_replay_plan.load(ROOT, PLAN)
    expected = plan.producer
    require(expected['revision'] == 'a2855f01e81002f24233399afb13b98849b45d00'
            and expected['origin_run'] == 42186797442, 'the artifact belongs to another producer')
    require(corpus_gate.host_target() == expected['target'] == 'linux_x86_64',
            'the retained artifact needs its original Linux host')
    require(subprocess.check_output(['rustc', '-Vv'], text=True).strip() == expected['toolchain'],
            'the retained artifact needs its original Rust toolchain')
    require(not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT),
            'the replay consumer must be clean')
    metadata = json.loads(subprocess.check_output(
        ['gh', 'api', f'repos/{REPOSITORY}/actions/artifacts/{ARTIFACT}']))
    require(metadata['id'] == ARTIFACT and metadata['name'] == ARTIFACT_NAME
            and metadata['size_in_bytes'] == ARTIFACT_BYTES and not metadata['expired']
            and metadata['digest'] == 'sha256:' + ARTIFACT_DIGEST
            and metadata['workflow_run']['id'] == expected['origin_run']
            and metadata['workflow_run']['head_sha'] == expected['revision'],
            'the retained artifact metadata changed')
    cache = Path(os.environ['CARGO_TARGET_DIR'])
    require(cache.is_absolute(), 'the relocation needs an explicit cache home')
    output = ROOT / 'build/compiler-pilot'
    output.mkdir(parents=True, exist_ok=True)
    report = output / 'artifact-relocation.json'
    require(not report.exists(), 'a relocation report already exists')
    with tempfile.TemporaryDirectory(prefix='omega-a285-artifact-') as temporary:
        temporary = Path(temporary)
        archive = temporary / 'producer.zip'
        with archive.open('xb') as destination:
            subprocess.run(['gh', 'api', f'repos/{REPOSITORY}/actions/artifacts/{ARTIFACT}/zip'],
                           stdin=subprocess.DEVNULL, stdout=destination, check=True)
        require(archive.stat().st_size == ARTIFACT_BYTES and digest(archive) == ARTIFACT_DIGEST,
                'the downloaded artifact does not match the declared archive digest')
        copied = []
        with zipfile.ZipFile(archive) as bundle:
            entries = {}
            for entry in bundle.infolist():
                relative = PurePosixPath(entry.filename)
                require(entry.filename and relative.as_posix() == entry.filename.rstrip('/')
                        and not relative.is_absolute() and '\\' not in entry.filename
                        and all(part not in ('.', '..') for part in relative.parts)
                        and '\x00' not in entry.filename and entry.filename not in entries,
                        'the artifact has an unsafe or repeated path')
                mode = entry.external_attr >> 16
                require(not stat.S_ISLNK(mode) and not (mode and
                        stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR)),
                        'the artifact has a non-regular payload')
                entries[entry.filename] = entry
            require('preparation.json' in entries and 'producer.json' in entries,
                    'the artifact lacks its actual producer records')
            for name in ('preparation.json', 'producer.json'):
                require(entries[name].file_size < 2 * 1024 * 1024,
                        'the artifact producer record exceeds its bounded metadata size')
            preparation = json.loads(bundle.read('preparation.json'))
            manifest = json.loads(bundle.read('producer.json'))
            require(preparation['passed'] is True and preparation['candidate'] == expected['revision']
                    and preparation['tree'] == expected['tree']
                    and preparation['producer'] == manifest['producer']
                    and manifest['schema'] == 'omega-compiler-producer-v1',
                    'the actual preparation did not retain this exact producer')
            producer = manifest['producer']
            require(all(producer[key] == expected[key]
                        for key in ('revision', 'tree', 'target', 'toolchain')),
                    'the artifact producer source/host identity differs')
            for kind, filename in (('cli', 'omega'), ('runner', 'corpus_runner')):
                identity = expected['executables'][kind]
                require({key: producer[kind + '_profile'].get(key) for key in ('sha256', 'profile')}
                        == identity, 'the actual producer profile differs')
                original = producer[kind]
                prefix = '/build/compiler-pilot/'
                require(original.count(prefix) == 1, 'the producer path has no unique retained home')
                relative = original.split(prefix, 1)[1]
                require(PurePosixPath(relative).name == filename and relative in entries
                        and 0 < entries[relative].file_size <= 600_000_000,
                        'the artifact executable has no bounded regular member')
                target = cache / 'debug' / filename
                target.parent.mkdir(parents=True, exist_ok=True)
                require(not target.exists() and not target.is_symlink(),
                        'the relocation would overwrite an existing executable')
                with bundle.open(entries[relative]) as source, target.open('xb') as destination:
                    while chunk := source.read(1024 * 1024):
                        destination.write(chunk)
                require(digest(target) == identity['sha256'], 'the artifact executable digest differs')
                target.chmod(0o755)
                sidecar = relative + '.compiler-profile.json'
                if sidecar in entries:
                    require(entries[sidecar].file_size < 2 * 1024 * 1024,
                            'the artifact profile sidecar exceeds its metadata bound')
                    Path(str(target) + '.compiler-profile.json').write_bytes(bundle.read(sidecar))
                observed = corpus_gate.require_optimized(target)
                require({key: observed.get(key) for key in ('sha256', 'profile')} == identity,
                        'the relocated executable profile differs')
                copied.append(dict(kind=kind, original=original, member=relative,
                                   relocated=str(target), sha256=identity['sha256'],
                                   observed_profile=observed))
        verified, profiles = compiler_replay_plan.materialize(
            ROOT, temporary, plan, corpus_gate.require_optimized)
        require(set(verified) == {'cli', 'runner'}, 'the existing materializer omitted a required binary')
        compiler_replay_plan.check_inputs(ROOT, 'HEAD', expected['input_objects'])
        # Keep the cache path roster identical to the existing restore step.
        for relative in ('registry', 'git'):
            (Path.home() / '.cargo' / relative).mkdir(parents=True, exist_ok=True)
        report.write_text(json.dumps(dict(
            schema='omega-retained-artifact-relocation-v1', original_producer=expected,
            artifact=metadata, archive_sha256=ARTIFACT_DIGEST, copied=copied,
            consumer_revision=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT,
                                                       text=True).strip(),
            existing_materialization_profiles=profiles, compiler_build_attempted=False,
            note='Only authenticated executable/profile bytes are relocated into a new cache entry; '
                 'the original producer revision and profiles remain unchanged.'), indent=2) + '\n')
        print('Authenticated retained a285 artifact; executable digests and profiles unchanged.')


if __name__ == '__main__':
    main()
