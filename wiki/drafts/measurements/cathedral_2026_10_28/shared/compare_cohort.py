import importlib.util,json,os,sys,time
from pathlib import Path
sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import capture_compile,require_optimized,settle_package_review
from cathedral import digest,verify_snapshot,validate_artifacts
base=Path(r'C:/SoftwareDevelopmentKits/Omega/.agents/worktree/perf-spill-round/build/cathedral-baseline')
snapshot=base/'Cathedral-syntax-migration'
compiler=Path('build/cathedral-benchmark/omega-608307c1ee2.exe').resolve()
cohort=Path('build/cathedral-benchmark/cohort').resolve()
directory=Path('build/cathedral-benchmark/cohort-comparison-02').resolve()
directory.mkdir(exist_ok=False)
entries=[{'name': 'ui-layout', 'source': 'source/distribution/libraries/omega-ui/tests/native_component/layout/main.omg', 'target': 'linux_x86_64', 'kind': 'component'}, {'name': 'ui-frame', 'source': 'source/distribution/libraries/omega-ui/tests/native_component/frame/main.omg', 'target': 'linux_x86_64', 'kind': 'component'}]
spec=importlib.util.spec_from_file_location('cathedral_elf',snapshot/'tools/omega-component/elf.py')
elf=importlib.util.module_from_spec(spec);spec.loader.exec_module(elf)
manifest=json.loads((base/'syntax-migration-source-manifest.json').read_text())
verify_snapshot(snapshot,manifest)
record=dict(compiler=require_optimized(compiler),source_manifest_sha256=digest(base/'syntax-migration-source-manifest.json'),cohort_sources={n:(cohort/n).read_text() for n in ['main.omg','build.omg']},runs=[])
def save(): (directory/'result.json').write_text(json.dumps(record,indent=2),encoding='utf-8')
save()
os.environ['OMEGA_PHASE_PROFILE']='0'
for project in [(snapshot/e['source']).parent for e in entries]+[cohort]:
    print('Preparing '+str(project),flush=True);settle_package_review(compiler,project,'linux_x86_64')
expected=None
for index,mode in enumerate(['pair','cohort','pair','cohort','cohort','pair']):
    sources=[snapshot/e['source'] for e in entries] if mode=='pair' else [cohort/'main.omg']
    row=dict(mode=mode,warmup=index<2,compiles=[],artifacts={})
    record['runs'].append(row)
    for number,source in enumerate(sources):
        output=directory/f'{index}-{mode}'/str(number)
        command=[str(compiler),'--offline','--timings','--target','linux_x86_64','--build-dir',str(output),str(source)]
        code,wall,peak,log=capture_compile(command,str(source.parent))
        (directory/f'{index}-{mode}-{number}.log').write_text(log,encoding='utf-8')
        row['compiles'].append(dict(command=command,exit_code=code,wall_ms=wall,peak_bytes=peak));save()
        assert code==0,log[-3000:]
        dependencies=elf.Elf((output/'omega-program').read_bytes(),2).needed()
        assert len(dependencies)==(1 if mode=='pair' else 2)
        for dependency in dependencies:
            name=dependency.decode().removeprefix('$ORIGIN/')
            assert dependency.decode()=='$ORIGIN/'+name and Path(name).name==name
            component=output/name
            elf.component(component.read_bytes())
            for path in [component,component.with_suffix('.so.component')]:
                assert path.name not in row['artifacts']
                row['artifacts'][path.name]=dict(sha256=digest(path),bytes=path.stat().st_size)
    row['wall_ms']=sum(c['wall_ms'] for c in row['compiles'])
    row['peak_bytes']=max(c['peak_bytes'] for c in row['compiles'])
    if expected is None: expected=row['artifacts']
    assert row['artifacts']==expected
    save();print(index,mode,row['wall_ms'],row['peak_bytes'],flush=True)
verify_snapshot(snapshot,manifest)
for name,contents in record['cohort_sources'].items():
    assert (cohort/name).read_text()==contents
assert digest(compiler)==record['compiler']['sha256']
record['inputs_unchanged']=True;save()
