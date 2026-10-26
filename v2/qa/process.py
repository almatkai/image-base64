#!/usr/bin/env python3
"""Real SDK binary/JSONL pipes, all writes and child homes under this clone."""
from pathlib import Path
import os,sys,json,uuid,select,subprocess,time,base64,hashlib
ROOT=Path(__file__).resolve().parents[1]
QA=ROOT/'.qa/pipes';QA.mkdir(parents=True,exist_ok=True)
BIN=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else ROOT/'target/debug/image-base64'
assert BIN.is_relative_to(ROOT), 'QA binary must be clone-owned'
PACKAGE=BIN.parent.parent if BIN.parent.name=='bin' else ROOT
checks=0
class Pipe:
 def __init__(self,name):
  self.root=QA/name;self.root.mkdir(exist_ok=True)
  for d in ['home/Downloads','temp','data','selected']:(self.root/d).mkdir(parents=True,exist_ok=True)
  env=dict(os.environ,HOME=str(self.root/'home'),USERPROFILE=str(self.root/'home'),TMPDIR=str(self.root/'temp'),TEMP=str(self.root/'temp'),TMP=str(self.root/'temp'))
  self.p=subprocess.Popen([str(BIN)],cwd=ROOT,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
  self.buffer=b'';self.queue=[];self.reverse=[]
  hello=self.read();assert hello['type']=='hello' and hello['id']=='com.tezbar.native.image-base64' and hello['sdkVersion']=='0.1.1'
  self.call('initialize',{'protocol':1,'extensionId':hello['id'],'platform':'macOS','packageDir':str(PACKAGE),'dataDir':str(self.root/'data'),'tempDir':str(self.root/'temp'),'grantedCapabilities':['clipboard.write']})
 def send(self,m):self.p.stdin.write(json.dumps(m,separators=(',',':')).encode()+b'\n');self.p.stdin.flush()
 def request(self,method,params):
  ident='h:'+str(uuid.uuid4());self.send({'type':'request','id':ident,'method':method,'params':params});return ident
 def read(self,timeout=5):
  deadline=time.monotonic()+timeout
  while b'\n' not in self.buffer:
   remaining=deadline-time.monotonic()
   assert remaining>0,'process read timeout'
   ready,_,_=select.select([self.p.stdout],[],[],remaining);assert ready,'process read timeout'
   data=os.read(self.p.stdout.fileno(),65536);assert data,'unexpected EOF: '+self.p.stderr.read().decode();self.buffer+=data
  line,self.buffer=self.buffer.split(b'\n',1);return json.loads(line)
 def response(self,ident):
  while True:
   for n,m in enumerate(self.queue):
    if m.get('type')=='response' and m.get('id')==ident:return self.queue.pop(n)
   m=self.read()
   if m.get('type')=='request':self.reverse.append(m)
   if m.get('type')=='response' and m.get('id')==ident:return m
   self.queue.append(m)
 def call(self,method,params):
  response=self.response(self.request(method,params));assert 'result'in response,response;return response['result']
 def open(self,sid,command='base64-to-image'):return self.call('session.open',{'sessionId':sid,'commandName':command,'preferences':{},'argumentValues':{}})
 def invoke(self,sid,action,fields=None):
  p={'actionId':action}
  if fields is not None:p['formValues']=fields
  return self.call('session.call',{'sessionId':sid,'operation':'extension:invoke-action','payload':p})
 def start_action(self,sid,action):return self.request('session.call',{'sessionId':sid,'operation':'extension:invoke-action','payload':{'actionId':action}})
 def reverse_request(self):
  while True:
   m=self.read()
   if m.get('type')=='request':self.reverse.append(m);assert m['method']=='host.clipboard.write' and list(m['params'])==['text'];return m
   self.queue.append(m)
 def reply(self,m,value):self.send({'type':'response','id':m['id'],'result':value})
 def refresh(self,sid):return self.call('session.call',{'sessionId':sid,'operation':'extension:refresh-session','payload':{}})
 def dispose(self,sid):return self.call('session.dispose',{'sessionId':sid})
 def close(self,eof=False):
  if eof:self.p.stdin.close()
  else:self.call('shutdown',{});self.p.stdin.close()
  self.p.wait(timeout=5);assert self.p.returncode==0,self.p.stderr.read().decode()
  assert not list((self.root/'temp').iterdir()),'owned temporary files leaked'
  assert not list((self.root/'home/Downloads').glob('.image-base64-*.tmp')),'destination staging files leaked'
def fields(v,rev,**values):return {**values,'__formToken':v['root']['props']['formToken'],'__formRevision':rev}
def action(v,kind):return next(a['id']for a in v['actions']if a['id'].endswith(':'+kind))
def metadata(v,title):return next(x['props']['text']for x in v['root']['metadata']['children']if x.get('props',{}).get('title')==title)
def submit(p,sid,v,text,rev=1,fmt='png'):return p.invoke(sid,v['actions'][0]['id'],fields(v,rev,base64=text,outputFormat=fmt))
def repeat_copy(p,sid,v,expected,title):
 global checks
 old=action(v,'copy');before=len(p.reverse)
 refreshed=p.refresh(sid);assert action(refreshed,'copy')==old
 req=p.start_action(sid,old);rpc=p.reverse_request();assert rpc['params']=={'text':expected}
 p.reply(rpc,{'ok':True});v=p.response(req)['result']
 fresh=action(v,'copy');assert fresh!=old and v['actions'][-1]['title']==title
 assert p.invoke(sid,old)['mode']=='unchanged';assert p.refresh(sid)==v
 assert len(p.reverse)==before+1;checks+=6
 req=p.start_action(sid,fresh);rpc2=p.reverse_request();assert rpc2['params']==rpc['params']
 p.reply(rpc2,{'ok':True});v=p.response(req)['result']
 assert action(v,'copy')not in [old,fresh] and v['actions'][-1]['title']==title
 assert p.invoke(sid,old)['mode']=='unchanged' and p.invoke(sid,fresh)['mode']=='unchanged'
 assert p.refresh(sid)==v and len(p.reverse)==before+2;checks+=5
 return v
p=Pipe('main')
v=p.open('base');f=submit(p,'base',v,'bad');assert f['root']['children'][0]['props']['value']=='bad';assert f['root']['type']=='Form';checks+=2
v=submit(p,'base',f,'SGVsbG8=',2,'jpeg');assert metadata(v,'Dimensions')=='0x0';assert v['root']['props']['markdown'].startswith('Preview unavailable');checks+=2
# Every ambiguous host response requires a NEW explicit exact-byte retry.
invalid=[None,True,False,{}, {'ok':False},{'ok':True,'error':'contradiction'},{'ok':True,'message':None},{'ok':True,'unknown':0}]
for value in invalid:
 old=action(v,'copy');req=p.start_action('base',old);rpc=p.reverse_request();assert rpc['params']['text']=='data:image/jpeg;base64,SGVsbG8='
 p.reply(rpc,value);v=p.response(req)['result'];assert v['actions'][-1]['title']=='Retry Copy Exact Base64';assert v['message'].startswith('Could not confirm copy');(QA/'unknown-copy-view.json').write_text(json.dumps(v));assert p.invoke('base',old)['mode']=='unchanged';checks+=3
# Overlap and busy refresh must have no new host effect.
req=p.start_action('base',action(v,'copy'));rpc=p.reverse_request();busy=p.response(p.start_action('base',action(v,'copy')));assert 'error'in busy;assert p.refresh('base')['mode']=='unchanged';checks+=2
p.send({'type':'cancel','requestId':req,'reason':'user cancelled'});cancelled=p.response(req);assert 'error'in cancelled
# Read cancel forwarded by SDK, then deliver a late host ACK; no success authority.
while True:
 queued=next((n for n,m in enumerate(p.queue) if m.get('type')=='cancel' and m.get('requestId')==rpc['id']),None)
 if queued is not None:p.queue.pop(queued);break
 m=p.read()
 if m.get('type')=='cancel' and m['requestId']==rpc['id']:break
 p.queue.append(m)
p.reply(rpc,{'ok':True});v=p.refresh('base');assert v['actions'][-1]['title']=='Retry Copy Exact Base64';checks+=2
req=p.start_action('base',action(v,'copy'));rpc2=p.reverse_request();assert rpc2['params']==rpc['params'];p.reply(rpc2,{'ok':True,'message':'copied'});v=p.response(req)['result'];assert v['actions'][-1]['title']=='Copy Image Base64';checks+=2
v=repeat_copy(p,'base',v,rpc2['params']['text'],'Copy Image Base64')
# Explicit download failure consumes its ID and permits a fresh deliberate retry.
original_download=action(v,'download');out=p.root/'home/Downloads';held=p.root/('held-downloads-'+uuid.uuid4().hex)
out.rename(held);out.write_bytes(b'blocked ordinary directory')
v=p.invoke('base',original_download);assert 'Download failed. Retry explicitly:'in v['message'];assert action(v,'download')!=original_download
assert p.invoke('base',original_download)['mode']=='unchanged';assert p.refresh('base')==v;checks+=4
out.unlink();held.rename(out)
# A raw-byte Download succeeds even when the captured SDK temp is unusable.
# This simulates the irrelevant temp-volume boundary without mounting a volume.
sdktemp=p.root/'temp';heldtemp=p.root/('held-temp-'+uuid.uuid4().hex);sdktemp.rename(heldtemp);sdktemp.write_bytes(b'not a directory')
download=action(v,'download');v=p.invoke('base',download);dest=Path(metadata(v,'Downloaded To'));assert dest.parent==out;assert dest.read_bytes()==b'Hello'
assert sdktemp.read_bytes()==b'not a directory' and not list(heldtemp.iterdir());sdktemp.unlink();heldtemp.rename(sdktemp);checks+=3
before=list(dest.parent.iterdir());dest.write_bytes(b'marker');assert p.invoke('base',download)['mode']=='unchanged';p.refresh('base');assert dest.read_bytes()==b'marker' and before==list(dest.parent.iterdir());checks+=2
# Repeated copies retain the acknowledged download checkpoint and frozen bytes.
v=repeat_copy(p,'base',v,'data:image/jpeg;base64,SGVsbG8=','Copy Image Base64')
assert metadata(v,'Downloaded To')==str(dest) and all(not a['id'].endswith(':download')for a in v['actions'])
assert p.invoke('base',download)['mode']=='unchanged' and dest.read_bytes()==b'marker' and before==list(dest.parent.iterdir());checks+=2
# Convert Another preserves exact original drafts and invalidates detail IDs.
old=action(v,'another');rpc_count=len(p.reverse);form=p.invoke('base',old);assert form['root']['children'][0]['props']['value']=='SGVsbG8=';assert p.invoke('base',old)['mode']=='unchanged';assert len(p.reverse)==rpc_count and before==list(dest.parent.iterdir()) and dest.read_bytes()==b'marker';checks+=3
p.dispose('base')
# Selected file conversion uses frozen bytes after unlink; hostile extension/name cannot enter Markdown.
png=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+ip1sAAAAASUVORK5CYII=')
selected=p.root/'selected'/'evil ]<img src=x onerror=bad>.JPG';selected.write_bytes(png)
f=p.open('file','image-to-base64');v=p.invoke('file',f['actions'][0]['id'],fields(f,1,filePath=[str(selected)]));assert metadata(v,'File Name')==selected.name;assert metadata(v,'Dimensions')=='1x1';assert v['root']['props']['markdown'].startswith('![Converted Image](data:image/png;base64,');selected.unlink();checks+=3
expected='data:image/JPG;base64,'+base64.b64encode(png).decode()
# File-command invalid ACK, consumed retry, canceled RPC and late ACK preserve exact text.
old=action(v,'copy');req=p.start_action('file',old);rpc=p.reverse_request();assert rpc['params']['text']==expected
p.reply(rpc,{'ok':True,'extra':False});v=p.response(req)['result'];assert v['actions'][-1]['title']=='Retry Copy Exact Base64';assert p.invoke('file',old)['mode']=='unchanged';checks+=3
retry=action(v,'copy');req=p.start_action('file',retry);rpc=p.reverse_request();assert rpc['params']['text']==expected
p.send({'type':'cancel','requestId':req,'reason':'user cancelled'});assert 'error'in p.response(req)
while True:
 queued=next((n for n,m in enumerate(p.queue)if m.get('type')=='cancel'and m.get('requestId')==rpc['id']),None)
 if queued is not None:p.queue.pop(queued);break
 m=p.read()
 if m.get('type')=='cancel'and m.get('requestId')==rpc['id']:break
 p.queue.append(m)
p.reply(rpc,{'ok':True});v=p.refresh('file');assert v['actions'][-1]['title']=='Retry Copy Exact Base64' and action(v,'copy')!=retry
assert p.invoke('file',retry)['mode']=='unchanged';checks+=4
v=repeat_copy(p,'file',v,expected,'Copy Base64 to Clipboard')
rpc_count=len(p.reverse);before=list((p.root/'home/Downloads').iterdir());form=p.invoke('file',action(v,'another'))
assert form['root']['children'][0]['props']['value']==[str(selected)] and len(p.reverse)==rpc_count and before==list((p.root/'home/Downloads').iterdir());checks+=1
p.dispose('file')
# Symlink/read errors do not lose selected draft. No silent import/selection.
f=p.open('errors','image-to-base64');missing=p.root/'selected'/'missing.png';v=p.invoke('errors',f['actions'][0]['id'],fields(f,1,filePath=[str(missing)]));assert v['root']['type']=='Form' and v['root']['children'][0]['props']['value']==[str(missing)];checks+=1;p.dispose('errors')
# Actual executable-generated view fixtures for the renderer boundary.
views=[]
for filename in ['tiny.png','tiny.jpeg','tiny.webp','lossless.webp']:
 selected=p.root/'selected'/filename;selected.write_bytes((ROOT/'qa/fixtures/images'/filename).read_bytes())
 form=p.open('preview','image-to-base64');view=p.invoke('preview',form['actions'][0]['id'],fields(form,1,filePath=[str(selected)]))
 assert view['root']['props']['markdown'].startswith('![Converted Image](data:image/');views.append(view);checks+=1;p.dispose('preview')
(QA/'native-views.json').write_text(json.dumps(views))
# One-use failed submit ID, dropdown injection, all-field A-B-A fencing.
f=p.open('fence');bad=submit(p,'fence',f,'bad');assert p.invoke('fence',f['actions'][0]['id'],fields(f,2,base64='AAAA',outputFormat='png'))['mode']=='unchanged';checks+=1
v=p.invoke('fence',bad['actions'][0]['id'],fields(bad,1,base64='AAAA',outputFormat='../x'));assert v['root']['type']=='Form';checks+=1
for rev,text in [(1,'AAAA'),(2,'BBBB'),(3,'AAAA')]:
 token=v['root']['props']['formToken'];v=p.invoke('fence','f:'+token+':change',fields(v,rev,base64=text,outputFormat='webp'))
stale=p.invoke('fence',v['actions'][0]['id'],fields(v,1,base64='AAAA',outputFormat='webp'));assert stale['mode']=='unchanged';checks+=1;p.dispose('fence')
# Per-session cache cap is explicit and recovers on dispose.
for n in range(16):p.open('cap'+str(n))
over=p.response(p.request('session.open',{'sessionId':'overflow','commandName':'base64-to-image'}));assert 'error'in over;checks+=1
for n in range(16):p.dispose('cap'+str(n))
p.close()
# 140 immediate canceled opens. Batch each request+cancel into a single pipe write.
p=Pipe('cancel-opens');cancel_count=0;success_count=0
for n in range(140):
 ident='h:'+str(uuid.uuid4());sid='cancel'+str(n)
 frames=[{'type':'request','id':ident,'method':'session.open','params':{'sessionId':sid,'commandName':'base64-to-image'}},{'type':'cancel','requestId':ident,'reason':'canceled open'}]
 p.p.stdin.write(b''.join(json.dumps(x).encode()+b'\n'for x in frames));p.p.stdin.flush();r=p.response(ident)
 if 'error'in r:
  assert 'cancel'in r['error']['message'].lower(),r;cancel_count+=1
 else:success_count+=1;p.dispose(sid)
assert cancel_count>0
v=p.open('recovered');assert v['root']['type']=='Form';checks+=141;p.dispose('recovered');p.close()
# EOF while reverse RPC is pending; no timers/tasks or temp leak.
p=Pipe('eof');f=p.open('s');v=submit(p,'s',f,'AAAA');p.start_action('s',action(v,'copy'));p.reverse_request();p.close(eof=True);checks+=1
print(json.dumps({'checks':checks,'immediateCanceledOpenCycles':140,'actuallyCancelled':cancel_count,'racedSuccessfulOpensDisposed':success_count,'recovered':True,'strictAckShapes':len(invalid),'repeatCopyCommands':2,'unusableSdkTempDownload':True,'explicitDownloadFailureRecovery':True,'realGui':False,'realUserFilesystemEffects':0,'binary':str(BIN),'sha256':hashlib.sha256(BIN.read_bytes()).hexdigest()}))
