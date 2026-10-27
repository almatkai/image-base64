#!/usr/bin/env python3
"""Record final Git-visible evidence and a complete nonrecursive hash inventory."""
from pathlib import Path
import subprocess,json,hashlib,datetime
r=Path(__file__).resolve().parents[1];root=r.parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert head=='317da99b58e2603444b3dfb9de7e97d6987f8ca7'
assert subprocess.check_output(['git','diff','--cached','--name-only'],cwd=root)==b''
assert subprocess.check_output(['git','diff','--name-only'],cwd=root,text=True).splitlines()==['README.md']
assert subprocess.check_output(['git','branch','--show-current'],cwd=root,text=True).strip()=='feat/native-v2-rust'

def paths():
 names=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=root).decode().split('\0')
 names=sorted(set(n for n in names if n))
 assert all(not ({'.qa','target','node_modules'}&set(Path(n).parts))and not n.endswith('.zip') for n in names)
 assert all(n=='README.md' or n.startswith('v2/') or (root/n).read_bytes()==subprocess.check_output(['git','show','HEAD:'+n],cwd=root) for n in names)
 return names
verification=json.loads((r/'.qa/verification.json').read_text())
assert all(t['exit']==0 or (t['name']=='windows-gnu-link'and t['exit']==101)for t in verification['tests'])
build=json.loads((r/'.qa/fresh-build.json').read_text());package=json.loads((r/'.qa/package.json').read_text())
assert package['executableSha256']==build['sha256'];assert package['processSmoke']['checks']==243
assert package['processSmoke']['repeatCopyCommands']==2
assert package['processSmoke']['unusableSdkTempDownload'] and package['processSmoke']['explicitDownloadFailureRecovery']
assert package['processSmoke']['actuallyCancelled']>0
assert sha(r/package['zip'])==package['sha256']
for p in (r/'src').glob('*.rs'):assert p.read_bytes()==(r/build['work']/'source/src'/p.name).read_bytes()
assert (r/'Cargo.lock').read_bytes()==(r/build['work']/'source/Cargo.lock').read_bytes()
subprocess.run(['git','apply','--check','--directory=v2/qa/fixtures/host','v2/integration/host-prerequisites.patch'],cwd=root,check=True)
subprocess.run(['git','diff','--check'],cwd=root,check=True)
source_hashes={n:sha(root/n)for n in paths()if n not in ['v2/qa/evidence.json','v2/INDEX.sha256']}
report={'recordedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sourceHead':head,'branch':'feat/native-v2-rust','sourceFilesSha256':source_hashes,'iconSha256':sha(r/'resources/extension-icon.png'),'verification':verification,'freshRelease':build,'devPackage':package,'xerj':json.loads((r/'.qa/xerj-final.json').read_text()),'provenance':json.loads((r/'.qa/provenance/provenance.json').read_text()),'handoff':{'uncommitted':True,'emptyIndex':True,'trackedV1UnchangedExceptReadmeDiscoveryLink':True,'hostPatch':'unapplied; git apply --check passed against frozen fixture','noCommitPushReleaseInstallOrCloneDeletion':True},'limits':{'inputUtf8Bytes':1500000,'fileDecodedBytes':1000000,'sessions':16,'previewMaxSide':8192,'previewMaxPixels':16000000},'platformAcceptance':{'macOS':'not-run','Windows':'not-run','crossVolumeDownloads':'not-run'},'reviewRound3':{'destinationOwnedStaging':True,'sdkTempDependencyRemoved':True,'atomicNoOverwriteCandidates':16,'freshDeliberateCopyAfterSuccessBothCommands':True,'oldIdsConsumed':True,'downloadCheckpointAcrossCopies':True,'simulatedUnusableSdkTemp':True},'windowsLinkLimitation':'GNU check passed; linking unavailable: missing x86_64-w64-mingw32-dlltool; MSVC target/toolchain unavailable'}
(r/'qa/evidence.json').write_text(json.dumps(report,indent=2)+'\n')
all_paths=paths();index=[f'{sha(root/n)}  {n}'for n in all_paths if n!='v2/INDEX.sha256']
(r/'INDEX.sha256').write_text('\n'.join(index)+'\n')
for line in index:
 digest,name=line.split('  ',1);assert sha(root/name)==digest
print(json.dumps({'head':head,'indexEmpty':True,'hashInventoryFiles':len(index),'evidenceSha256':sha(r/'qa/evidence.json'),'inventorySha256':sha(r/'INDEX.sha256'),'allowedChangesOnly':True,'zip':package['zip'],'zipSha256':package['sha256'],'executableSha256':build['sha256']}))
