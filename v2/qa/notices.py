#!/usr/bin/env python3
"""Copy authentic locked dependency notices from this clone's Cargo cache."""
from pathlib import Path
import subprocess,json,os,hashlib,shutil
r=Path(__file__).resolve().parents[1]
env=dict(os.environ,CARGO_HOME=str(r/'.qa/cargo'),TMPDIR=str(r/'.qa/tmp'))
m=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1','--manifest-path',str(r/'Cargo.toml')],env=env))
rows=[]
for package in sorted(m['packages'],key=lambda p:(p['name'],p['version'])):
 if package['name']=='image-base64-native':continue
 source=Path(package['manifest_path']).parent
 assert source.is_relative_to(r/'.qa/cargo')
 files=[]
 for p in sorted(source.iterdir()):
  if p.is_file() and p.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','UNLICENSE')):
   out=r/'resources/licenses'/f"{package['name']}-{package['version']}"/p.name;out.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,out)
   files.append({'path':str(out.relative_to(r)),'sha256':hashlib.sha256(out.read_bytes()).hexdigest()})
 rows.append({'name':package['name'],'version':package['version'],'license':package['license'],'source':package['source'],'notices':files})
# Preserve superseded generated notice versions in ignored QA evidence rather
# than shipping files for a dependency no longer present in the lock.
current={f"{x['name']}-{x['version']}"for x in rows}
for directory in (r/'resources/licenses').iterdir():
 if directory.is_dir() and directory.name not in current:
  import uuid
  destination=r/'.qa/superseded-notices'/(directory.name+'-'+uuid.uuid4().hex[:8]);destination.parent.mkdir(parents=True,exist_ok=True);shutil.move(str(directory),str(destination))
(r/'qa/dependency-notices.json').write_text(json.dumps(rows,indent=2)+'\n')
text='''# Notices and provenance

The Rust port is MIT licensed; see [LICENSE](LICENSE). The original own V1
[license](../LICENSE) remains unchanged. Raycast extension code/icon provenance:
`raycast/extensions` commit `6734d85992290066fd31a7f5bee3cca3d3545a1a`,
`extensions/image-base64`; authentic [Raycast MIT license](resources/RAYCAST-LICENSE).

The 262,271-byte original icon SHA256 is
`3ad7436debd7aa7bedfae7f081e25d0f16ba554e8ad9414f663625adc1024a43`.
It matches the original pinned upstream URL, own `assets/extension-icon.png`, and
`resources/extension-icon.png` byte for byte. No command-specific image icons are
declared by either source manifest. No icon has been regenerated/recompressed.

The SDK 0.1.1 is pinned to `e955911f0bf5b558f9d3eb5183df0adc95c22af2`;
its authentic Apache 2.0 [license](resources/SDK-LICENSE) is retained.
Host QA fixtures are exact Git-show bytes at `ea100cee3f69711de12485ba061950c5fc65258f`,
with the original [host license](qa/fixtures/host/LICENSE). Fixture hashes and
upstream icon/license URL provenance are recorded in [pins.json](qa/pins.json).

Authentic Cargo dependency license/notice files are copied under
[resources/licenses](resources/licenses). [dependency-notices.json](qa/dependency-notices.json)
records exact locked versions, declared SPDX license expressions, source references,
and copied-file hashes; this includes development and platform-only dependencies.
Crates distributing no separate license file are explicitly visible as empty
notice lists, without invented copyright text. Cargo's registry source checksum
and declared license remain recorded. Third-party copyright notices are unmodified.

Synthetic 7×9 raster fixtures in `qa/fixtures/images` were created locally using
Pillow (PNG/JPEG/WebP and lossless WebP). They contain a single solid color, with
no third-party artwork. Small crafted ZIP fixtures in `qa/fixtures/archives` are
local inert parser-test bytes (including intentionally invalid names/link modes).
Renderer QA dependency versions are frozen separately in
`qa/package-lock.json`; they are development tools and never enter the product ZIP.
'''
(r/'NOTICES.md').write_text(text)
print(json.dumps({'lockedDependencies':len(rows),'authenticNoticeFiles':sum(len(x['notices'])for x in rows),'withoutSeparateNotice':[x['name']for x in rows if not x['notices']]}))
