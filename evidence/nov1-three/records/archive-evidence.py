from pathlib import Path
import datetime, hashlib, json, os, re, shutil, subprocess

root = Path.cwd()
out = root / 'build/swarm/nov1'
submitted = '536864f05a5a1fcf7860c5eb72788fd8d78ba78e'
semantic = 'ea132c1fff8bccd603ac18adb83691ee4d2e8f23'
base = 'bfcffe6bcfcedb6fd60713e223408566c6c4e10d'
tag = 'evidence/nov1-three-536864f05a5'
assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip() == submitted
assert not subprocess.check_output(['git','status','--porcelain'],text=True).strip()
review = json.loads((out/'independent-final-review.json').read_text())
assert review['candidate'] == submitted and review['decision'] == 'accepted'
assert not review['blocking_findings']
stage = out/'evidence-stage'
stage.mkdir(exist_ok=False)
selected = {}

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        while chunk := stream.read(1024*1024):
            h.update(chunk)
    return h.hexdigest()

def add(source, relative):
    source = source.resolve()
    assert source.is_file()
    if relative in selected:
        assert selected[relative] == source
    selected[relative] = source

text_suffixes = {'.json','.jsonl','.stdout','.stderr','.txt','.py','.patch','.md','.omg','.lock'}
producer_names = {'GENERATION','RUNNER_GENERATION','READY','profile.json','runner-profile.json'}
for current, dirs, files in os.walk(out):
    dirs[:] = [d for d in dirs if d not in {'build','target','.git','.omega','__pycache__','evidence-stage'}]
    for name in files:
        p = Path(current)/name
        rel = p.relative_to(out)
        if p.is_symlink() or name in {'pr-body.md','evidence-publication.json'}:
            continue
        if 'producers' in rel.parts and name not in producer_names:
            continue
        if p.suffix == '.lock' and name != 'omega.lock':
            continue
        if p.suffix not in text_suffixes and name not in producer_names:
            continue
        # Archive only text plus the explicitly selected small Git bundles below.
        p.read_bytes().decode('utf-8')
        add(p, 'records/'+rel.as_posix())

baseline = root.parent/'nov1-coord/build/swarm/nov1/producers/gen-bfcffe6bcfc'
for name in ['GENERATION','READY','profiles.json']:
    add(baseline/name, 'baseline-producer/'+name)

bundle_revisions = [semantic,'24efd51f96451d5dc726e6b37d537fc7ca5aad4f','87c526e435e0e49b28fb217b158ba7c0db111f64']
for revision in bundle_revisions:
    add(root/'build/corpus-source'/f'{revision}.bundle', 'corpus-source/'+revision+'.bundle')

receipt_ids = ['0519eac619c642e8b7a152251574a554','76cd8ed27051431786a9d8b54c282ca9']
final_artifacts = {}
for rid in receipt_ids:
    receipt = json.loads((out/'cleanup-baseline.records'/f'{rid}.json').read_text())
    assert receipt['exit'] == 0 and not receipt['invalidations']
    assert receipt['revision'] == submitted
    for absolute, expected in receipt['artifacts'].items():
        if absolute in final_artifacts:
            assert final_artifacts[absolute]['sha256'] == expected
            continue
        p=Path(absolute)
        assert digest(p) == expected, absolute
        if p.name == 'omega.lock':
            archive='accepted-locks/'+p.relative_to(root).as_posix()
            add(p, archive)
            disposition='Archived for inspection; not reusable approval or authority'
        elif absolute == '/usr/bin/python3':
            archive=None
            disposition='System executable omitted; exact digest retained in receipt'
        elif p.name == 'omega':
            archive=None
            disposition='Compiler executable omitted; exact optimized digest/profile and build source retained'
        else:
            archive='records/'+p.relative_to(out).as_posix()
            assert archive in selected
            disposition='Archived verbatim'
        final_artifacts[absolute]={'sha256':expected,'archive':archive,'disposition':disposition}

# Nested support package locks were normal source acceptance inputs. They do not
# confer authority on a new checkout or compiler and are archival only.
for fact in ['open_const_implementation','open_record_const_implementation']:
    for current, dirs, files in os.walk(root/'samples/cli/facts'/fact):
        dirs[:] = [d for d in dirs if d not in {'build','target','.omega'}]
        if 'omega.lock' in files:
            p=Path(current)/'omega.lock'
            add(p, 'accepted-locks/'+p.relative_to(root).as_posix())

secret_patterns = [rb'gh[pousr]_[A-Za-z0-9]{30,}',rb'github_pat_[A-Za-z0-9_]{40,}',rb'sk-(?:proj-)?[A-Za-z0-9_-]{40,}',rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----']
manifest=[]
for relative, source in sorted(selected.items()):
    data=source.read_bytes()
    if not relative.endswith('.bundle'):
        for pattern in secret_patterns:
            assert not re.search(pattern,data), 'Credential-like bytes in '+relative
    target=stage/relative
    target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(source,target)
    manifest.append({'path':relative,'original_path':str(source),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})

readme='''# Selected const-contract repair: immutable execution evidence

This evidence tree is separate from the source PR. Its parent is the exact
independently accepted submitted source 536864f05a5a1fcf7860c5eb72788fd8d78ba78e.
The substantive compiler producer is ea132c1fff8bccd603ac18adb83691ee4d2e8f23;
its compiler/library/toolchain trees are identical to the submitted source.
The fixed original baseline is bfcffe6bcfcedb6fd60713e223408566c6c4e10d.
No historical observation is relabelled as an execution of another compiler.

Start with records/independent-final-review.json,
records/independent-semantic-review.json,
records/submitted-independent-review-context.stdout and
records/current-corpus-provenance.json. Final qualification state revision69
and the original immutable receipts are in records/cleanup-baseline.json and
records/cleanup-baseline.records/. Final receipts are
0519eac619c642e8b7a152251574a554 and 76cd8ed27051431786a9d8b54c282ca9.
The two qualified-*-submitted/observations.json files preserve all49 raw
source hashes/commands/streams:23 native0/Terminal0 positives and26 intended
refusals. The bounded comparisons are81 source and34 native/Terminal fixtures,
with zero losses and34 actual engine pairs0/agrees. The84-file inventory
includes three support package roots, not84 independently executed fixtures.

The original wrong-positive refusals, successive measured gaps and the
same-statement return-write false acceptance are retained unedited. Earlier
setup, formation and downstream-underflow probes are not intended-control
acceptance. Historical-probe-sources includes two exact old scratch sources
reconstructed by reversing recorded edits; their SHA256 matches the original
receipt artifact, and its provenance explains this. The old observation is
unchanged. Original-row snapshots and authored scratch sources remain readable.
Diagnostic instrumentation patches/logs describe historical diagnosis and are
absent from submitted compiler source. Old qualification/configuration and
frontier records remain historical records, not current approval.

The40 source,14 native and14 both recorded movements have their actual execution
source identities and all outside golden bytes preserved. The three small Git
bundles in corpus-source preserve those observed source revisions across
squash publication and branch cleanup. All earlier source commits are also
reachable through this archive's submitted-source parent. record-ledger.json,
source-golden-attribution.json and both-golden-attribution.json explain why
unchanged baseline diagnostic debt was left unchanged. Jev was unavailable and
supplied no approval. Scoped Clippy remains non-green on206 baseline-span
findings; affected crate checking and formatting pass.

Producer binaries, system executables, caches and generated runtime images
are omitted. Exact digests, optimized profiles, source closures and build logs
are retained. ARTIFACTS.json records the disposition of every consumed artifact
in the final receipts. Accepted omega.lock files are archived only for
inspection and may not be reused as new authority or approval. New acceptance
must use the compiler's ordinary review path. MANIFEST.json maps archive bytes
to their original absolute paths and SHA256 digests.

Native observations are Linux x86-64. Four hosted Build entries do not prove
other-host execution. Full STATIC-MACHINE-MIGRATION acceptance, other member
and calling kinds, product parity and source retirement remain open.
'''
(stage/'README.md').write_text(readme)
(stage/'ARTIFACTS.json').write_text(json.dumps(final_artifacts,indent=2)+'\n')
metadata={'schema':'omega-immutable-evidence-manifest-v1','submitted_source':submitted,'semantic_producer':semantic,'fixed_base':base,'tag':tag,'pr':1205,'created_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'payload':manifest}
(stage/'MANIFEST.json').write_text(json.dumps(metadata,indent=2)+'\n')
print(json.dumps({'stage':str(stage),'files':len(manifest)+3,'payload_bytes':sum(v['bytes'] for v in manifest),'final_artifacts':len(final_artifacts),'bundles':bundle_revisions,'secret_pattern_scan':'clear','tracked_tree_clean':not subprocess.check_output(['git','status','--porcelain'],text=True).strip()},indent=2))
