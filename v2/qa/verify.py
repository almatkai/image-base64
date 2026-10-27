#!/usr/bin/env python3
"""Clone-confined reproducible verification. No host worktree or OS UI effects."""
from pathlib import Path
import os,subprocess,json,hashlib,shutil,sys,time
r=Path(__file__).resolve().parents[1];root=r.parent;qa=r/'.qa'
for name in ['home','tmp','cargo','npm-cache','logs']:(qa/name).mkdir(parents=True,exist_ok=True)
# Keep toolchain binaries read-only in their existing installation; all mutable
# state belongs to this clone. No installers or toolchain downloads are invoked.
env=dict(os.environ,HOME=str(qa/'home'),USERPROFILE=str(qa/'home'),TMPDIR=str(qa/'tmp'),TMP=str(qa/'tmp'),TEMP=str(qa/'tmp'),CARGO_HOME=str(qa/'cargo'),RUSTUP_HOME=str(Path.home()/'.rustup'),npm_config_cache=str(qa/'npm-cache'),npm_config_update_notifier='false')
reports=[]
def run(name,args,cwd=root,allowed_failure=False):
 start=time.monotonic();p=subprocess.run(args,cwd=cwd,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 (qa/'logs'/f'{name}.log').write_text(p.stdout)
 report={'name':name,'command':args,'exit':p.returncode,'seconds':round(time.monotonic()-start,2),'logSha256':hashlib.sha256(p.stdout.encode()).hexdigest()}
 try: report['result']=json.loads(p.stdout.strip().splitlines()[-1])
 except (ValueError,IndexError):pass
 reports.append(report);print(name,p.returncode,p.stdout[-750:],flush=True)
 if p.returncode and not allowed_failure:raise RuntimeError(name+' failed')
 return p.stdout
assert subprocess.check_output(['git','rev-parse','--show-toplevel'],cwd=root,text=True).strip()==str(root)
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()=='317da99b58e2603444b3dfb9de7e97d6987f8ca7'
assert not subprocess.check_output(['git','diff','--cached','--name-only'],cwd=root)
baseline=json.loads((r/'qa/v1-baseline.json').read_text())
for file,digest in baseline.items():
 original=subprocess.check_output(['git','show','HEAD:'+file],cwd=root)
 assert hashlib.sha256(original).hexdigest()==digest,file
 if file!='README.md':assert (root/file).read_bytes()==original,file
 else:assert (root/file).read_bytes()==original+b'\n[Standalone Rust V2 source and verification](v2/README.md).\n'
# Prepare a disposable V1 source copy from exact HEAD, preserving its root lock.
v1=qa/'v1';v1.mkdir(exist_ok=True)
for file in baseline:
 p=v1/file;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(subprocess.check_output(['git','show','HEAD:'+file],cwd=root))
renderer=qa/'renderer';renderer.mkdir(exist_ok=True)
for name in ['package.json','package-lock.json']:shutil.copyfile(r/'qa'/name,renderer/name)
run('npm-v1',['npm','ci','--ignore-scripts','--no-audit','--no-fund'],v1)
run('v1-test',['npm','test'],v1);run('v1-typecheck',['npm','run','typecheck'],v1)
run('npm-renderer',['npm','ci','--ignore-scripts','--no-audit','--no-fund'],renderer)
run('source-oracle',['node',str(r/'qa/source-oracle.cjs')])
run('host-patch',['python3',str(r/'qa/host-patch.py')]);run('host-contract',['python3',str(r/'qa/host-contract.py')]);run('producers',['python3',str(r/'qa/producers.py')])
run('notices',['python3',str(r/'qa/notices.py')])
run('fresh-release-build',['python3',str(r/'qa/fresh-build.py')])
run('dev-package',['python3',str(r/'qa/package.py')])
run('fmt',['cargo','fmt','--manifest-path',str(r/'Cargo.toml'),'--','--check'])
for tool in ['stable','1.85.0']:
 run(tool+'-tests',['cargo','+'+tool,'test','--locked','--all-targets','--manifest-path',str(r/'Cargo.toml')])
 run(tool+'-clippy',['cargo','+'+tool,'clippy','--locked','--all-targets','--manifest-path',str(r/'Cargo.toml'),'--','-D','warnings'])
run('debug-build',['cargo','build','--locked','--manifest-path',str(r/'Cargo.toml')])
run('real-sdk-pipes',['python3',str(r/'qa/process.py')]);run('renderer',['node',str(r/'qa/renderer.cjs')]);run('schema',['node',str(r/'qa/schema.cjs')])
installed=run('installed-targets',['rustup','target','list','--installed'])
if 'x86_64-pc-windows-gnu' in installed:
 run('windows-gnu-check',['cargo','check','--locked','--target','x86_64-pc-windows-gnu','--manifest-path',str(r/'Cargo.toml')])
 output=run('windows-gnu-link',['cargo','build','--locked','--target','x86_64-pc-windows-gnu','--manifest-path',str(r/'Cargo.toml')],allowed_failure=True)
 if reports[-1]['exit']:assert "error calling dlltool 'x86_64-w64-mingw32-dlltool': No such file or directory" in output, 'Unexpected Windows link failure'
run('versions',['rustc','--version']);run('msrv-version',['rustc','+1.85.0','--version']);run('node-version',['node','--version'])
# Fixture authentication against exact immutable bytes already recorded in pins.
pins=json.loads((r/'qa/pins.json').read_text())
for file,digest in pins['fixtures'].items():assert hashlib.sha256((r/'qa/fixtures'/file).read_bytes()).hexdigest()==digest,file
sdk_paths=list((qa/'cargo/git/checkouts').glob('*/e955911'))
assert len(sdk_paths)==1,sdk_paths
for file in [p[4:]for p in pins['fixtures']if p.startswith('sdk/')]:
 original=subprocess.check_output(['git','-C',str(sdk_paths[0]),'show',pins['sdk']+':'+file])
 assert (r/'qa/fixtures/sdk'/file).read_bytes()==original,file
assert (v1/'package-lock.json').read_bytes()==(root/'package-lock.json').read_bytes()
report={'sourceHead':pins['source'],'hostPin':pins['host'],'sdkPin':pins['sdk'],'platformAcceptance':{'macOS':'not-run','Windows':'not-run'},'trackedV1Preserved':True,'indexEmpty':True,'tests':reports}
(qa/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print('Verification complete; .qa/verification.json')
