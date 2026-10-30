from pathlib import Path
import gzip,hashlib,json,shutil
ROOT=Path.cwd();OUT=ROOT/'build/proof-profile';WORK=OUT/'source-checking-cpu'
DEST=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/source_checking_cpu';DEST.mkdir(exist_ok=False)
for name in ['source.json','diagnostic.patch','research.md','input-manifest.json','producer.json','summary.json','patch-checks.json','post-merge-smoke.json','post-merge-smoke.log.gz','post-merge-smoke-extract.txt']:
    shutil.copyfile(WORK/name,DEST/name)
shutil.copyfile(OUT/'source-checking-cpu-update.log',DEST/'update.log')
shutil.copyfile(WORK/'run.log',DEST/'run.log')
for src,dst in [('prepare_check_cpu.py','prepare.py'),('check_cpu_run.py','run.py'),('check_cpu_analyze.py','analyze.py'),('restore_check_cpu.py','restore.py'),('freeze_core.py','freeze.py'),('check_cpu_smoke.py','smoke.py'),('archive_check_cpu.py','archive.py')]:shutil.copyfile(OUT/src,DEST/dst)
for result in sorted((WORK/'observations').glob('*.json')):
    label=result.stem;folder=OUT/label;raw=DEST/'raw'/label;raw.mkdir(parents=True)
    for name in ['result.json','stdout.txt','stderr.txt']:(raw/(name+'.gz')).write_bytes(gzip.compress((folder/name).read_bytes(),mtime=0))
    if (folder/'runtime').is_dir():
        (raw/'runtime').mkdir()
        for name in ['result.json','stdout.txt','stderr.txt']:(raw/'runtime'/(name+'.gz')).write_bytes(gzip.compress((folder/'runtime'/name).read_bytes(),mtime=0))
log=(WORK/'build.log').read_bytes();text=log.decode('utf-16' if log.startswith(b'\xff\xfe') else 'utf-8')
(DEST/'build.log.gz').write_bytes(gzip.compress(text.encode('utf-8'),mtime=0))
entries={p.relative_to(DEST).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in DEST.rglob('*') if p.is_file()}
(DEST/'manifest.json').write_text(json.dumps(entries,indent=2)+'\n',encoding='utf-8')
print('Archived',len(entries),'files at',DEST)
