import { spawn, execFileSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFileSync, writeFileSync, mkdirSync, copyFileSync, renameSync } from 'node:fs';
import { createHash, randomUUID } from 'node:crypto';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';
const candidate = JSON.parse(readFileSync('artifacts/candidates/latest-candidate.json','utf8'));
const installReceipt=JSON.parse(readFileSync(join(candidate.directory,'install-report.json'),'utf8').replace(/^\uFEFF/,''));
assert.equal(installReceipt.status,'passed','Exact candidate installation must pass first');
assert.equal(installReceipt.installerSha256,candidate.installer.sha256,'Installed candidate does not match the frozen installer');
const installationDirectory=installReceipt.installDirectory;
const binary=join(installationDirectory,'tinixcut.exe');
const sha=file=>createHash('sha256').update(readFileSync(file)).digest('hex');
assert.equal(sha(binary),candidate.expectedInstalledBinary.sha256);
const directory=join(candidate.directory,'acceptance',new Date().toISOString().replaceAll(':','-'));mkdirSync(directory,{recursive:true});
const report={startedAt:new Date().toISOString(),binary,binarySha256:sha(binary),directory,scope:'Exact installed candidate CLI, no GUI/history/recovery/file-dialog claim',steps:[],limitations:['CLI exposes no Undo/Redo, recovery or collect; these require a qualified GUI/native route.','Project fixtures are disposable and edits use installed Command Service.']};
const child=spawn(binary,['api-session'],{windowsHide:true,stdio:['pipe','pipe','pipe'],cwd:installationDirectory});
const lines=createInterface({input:child.stdout});const queue=[],waiters=[];let stderr='';let nextId=0;
lines.on('line',line=>{const value=JSON.parse(line);const waiting=waiters.shift();if(waiting)waiting(value);else queue.push(value);});
child.stderr.on('data',d=>{stderr+=d;});
const next=()=>queue.length?Promise.resolve(queue.shift()):new Promise(r=>waiters.push(r));
async function tool(name,args,expectError=false){
 const id=++nextId;child.stdin.write(JSON.stringify({id,command:name.replace(/^tinixcut_/,'') ,arguments:args})+'\n');
 let timer;const response=await Promise.race([next(),new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error(`CLI session timeout ${name}: ${stderr}`)),90000);})]).finally(()=>clearTimeout(timer));
 assert.equal(response.id,id);assert.equal(response.result?.isError,expectError,JSON.stringify(response));return response.result.structuredContent ?? response.result;
}
const read=file=>JSON.parse(readFileSync(file,'utf8'));
const time=value=>({value,rate:{numerator:30,denominator:1}});
const cli=(...args)=>execFileSync(binary,args,{encoding:'utf8',windowsHide:true,timeout:45000,cwd:installationDirectory});
const ffmpeg=join(installationDirectory,'resources/bin/ffmpeg.exe'),ffprobe=join(installationDirectory,'resources/bin/ffprobe.exe');
const sequences=new Map();
async function apply(file,operations,baseOverride){
 const p=read(file);const n=(sequences.get(p.project_id)??0)+1;
 const envelope={schema_version:p.schema_version,project_id:p.project_id,base_revision:baseOverride??p.revision,client_id:'installed-acceptance',client_epoch:1,sequence_number:n,transaction_id:`tx_${n}`,operations:operations.map((operation,i)=>({operation_id:`installed-acceptance:1:${n}:${i}`,operation}))};
 const result=await tool('tinixcut_apply',{projectPath:file,envelope},baseOverride!==undefined);
 if(baseOverride===undefined)sequences.set(p.project_id,n);
 return {envelope,result};
}
try{
 const project=join(directory,'project.tcproj');await tool('tinixcut_project_create',{projectPath:project,name:'Installed candidate acceptance'});
 report.capabilities=await tool('tinixcut_capabilities',{});
 const source=join(directory,'tone.wav');execFileSync(ffmpeg,['-v','error','-y','-f','lavfi','-i','sine=frequency=440:sample_rate=48000:duration=2','-ac','2',source],{windowsHide:true,timeout:10000});
 const probe=await tool('tinixcut_media_probe',{mediaPath:source});report.mediaProbe=probe;
 const p=read(project);const asset={asset_id:'tone',name:'tone.wav',path:source,media_kind:'Audio',duration:time(60),file_size:readFileSync(source).length,dimensions:null};
 // AssetImport is a command, not direct persistence mutation.
 await apply(project,[{type:'AssetImport',payload:asset},{type:'TrackCreate',payload:{track_id:'audio',name:'Audio',kind:'Audio'}},{type:'ClipCreate',payload:{track_id:'audio',item_id:'audio',name:'Tone',kind:'Audio',asset_id:'tone',timeline_start:time(0),duration:time(60),source_start:time(0)}}]);
 report.steps.push('Installed CLI create, probe, asset/track/clip commands and persisted reopen');
 const edit=await apply(project,[{type:'TrackVolumeSet',payload:{track_id:'audio',volume:1.9952623}}]);
 const after=read(project),beforeHash=sha(project);
 const retry=await tool('tinixcut_apply',{projectPath:project,envelope:edit.envelope});assert.equal(retry.operationsApplied,0);assert.equal(retry.newRevision,after.revision);assert.equal(sha(project),beforeHash);report.retry=retry;
 await apply(project,[{type:'TrackVolumeSet',payload:{track_id:'audio',volume:0.5}}],0);assert.equal(sha(project),beforeHash);
 const copy=join(directory,'saved-copy.tcproj');await tool('tinixcut_save',{projectPath:project,targetPath:copy});assert.deepEqual(read(copy).tracks,after.tracks);assert.equal(sha(project),beforeHash);
 cli('inspect',copy);report.steps.push('Installed retry/stale rejection preserves bytes; save-copy and CLI inspect reopen');
 const moved=join(directory,'tone-moved.wav');const sourceHash=sha(source);renameSync(source,moved);
 report.offlinePreflight=await tool('tinixcut_export_preflight',{projectPath:project,outputPath:join(directory,'offline.mp4')},true);
 await apply(project,[{type:'AssetRelink',payload:{asset_id:'tone',path:moved}}]);assert.equal(sha(moved),sourceHash);assert.equal(read(project).assets[0].path,moved);
 report.steps.push('Installed offline export rejection and scoped relink retain source bytes');
 const output=join(directory,'cli-export.mp4');report.preflight=await tool('tinixcut_export_preflight',{projectPath:project,outputPath:output});
 const job=await tool('tinixcut_export_start',{projectPath:project,outputPath:output});
 let status;for(let n=0;n<400;n++){status=await tool('tinixcut_export_status',{jobId:job.jobId});if(['completed','failed','cancelled'].includes(status.status))break;await new Promise(r=>setTimeout(r,150));}
 assert.equal(status.status,'completed',JSON.stringify(status));report.exportStatus=status;
 report.exportProbe=JSON.parse(execFileSync(ffprobe,['-v','error','-show_format','-show_streams','-of','json',output],{encoding:'utf8',windowsHide:true,timeout:10000}));
 execFileSync(ffmpeg,['-v','error','-i',output,'-f','null','-'],{windowsHide:true,timeout:20000});
 assert.ok(Number(report.exportProbe.format.duration)>=2);report.exportSha256=sha(output);
 report.steps.push('Installed CLI preflight/export/status, packaged ffprobe and full FFmpeg decode');
 const session=spawn(binary,['session'],{windowsHide:true,stdio:['pipe','pipe','pipe']});const sessionLines=createInterface({input:session.stdout});const iterator=sessionLines[Symbol.asyncIterator]();
 try{let id=0;for(const [method,params] of [['session.open',{path:copy}],['project.inspect',{}],['project.save',{}],['session.close',{}],['session.shutdown',{}]]){
  session.stdin.write(JSON.stringify({protocolVersion:1,requestId:`s${++id}`,method,params})+'\n');let t;const line=await Promise.race([iterator.next(),new Promise((_,reject)=>{t=setTimeout(()=>reject(new Error('Session timeout')),10000);})]).finally(()=>clearTimeout(t));assert.equal(JSON.parse(line.value).ok,true);
 }}finally{session.stdin.end();sessionLines.close();if(session.exitCode===null)session.kill();}
 report.steps.push('Installed CLI stdio session open/inspect/save/close without GUI');
 const caption=join(directory,'captions.tcproj');copyFileSync(resolve('scripts/qualification/background/caption-merge.tcproj'),caption);
 await apply(caption,[{type:'ClipTextSet',payload:{item_id:'caption_left',text:'Xin chào\nViệt Nam — corrected'}}]);assert.equal(read(caption).tracks[0].items[0].text_content,'Xin chào\nViệt Nam — corrected');
 const unchangedRight=read(caption).tracks[0].items[1];await apply(caption,[{type:'ClipTextSet',payload:{item_id:'caption_left',text:'Confirmed correction'}}]);assert.deepEqual(read(caption).tracks[0].items[1],unchangedRight);
 report.steps.push('Installed caption correction preserves another confirmed caption through Command Service');
 const speech=join(directory,'speech.wav');copyFileSync(resolve('artifacts/phase26-whisper/test-input-jfk.wav'),speech);
 const transcript=await tool('tinixcut_media_transcript',{mediaPath:speech,whisperCliPath:resolve('artifacts/phase26-whisper/whisper.cpp-v1.9.2-x64/Release/whisper-cli.exe'),modelPath:resolve('artifacts/phase26-whisper/models/ggml-base.bin'),language:'en'});
 report.transcript=transcript;assert.ok(transcript.result?.segments?.length>0,JSON.stringify(transcript));
 report.transcriptionInputs={sourceSha256:sha(speech),cliSha256:sha(resolve('artifacts/phase26-whisper/whisper.cpp-v1.9.2-x64/Release/whisper-cli.exe')),modelSha256:sha(resolve('artifacts/phase26-whisper/models/ggml-base.bin'))};
 const recognized=transcript.result.segments[0].text;
 await apply(caption,[{type:'ClipTextSet',payload:{item_id:'caption_left',text:recognized}}]);
 const confirmed=read(caption).tracks[0].items[1];
 await apply(caption,[{type:'ClipTextSet',payload:{item_id:'caption_left',text:recognized+' [corrected]'}}]);
 assert.equal(read(caption).tracks[0].items[0].text_content,recognized+' [corrected]');assert.deepEqual(read(caption).tracks[0].items[1],confirmed);cli('inspect',caption);
 report.steps.push('Installed recognized transcript text -> Command Service caption correction/reopen preserves another confirmed caption; GUI correction excluded');
 report.steps.push('Installed CLI real local Whisper transcript, no mocked recognition');
 assert.equal(sha(binary),candidate.expectedInstalledBinary.sha256);report.status='passed';
 console.log('PASS: installed candidate CLI acceptance');
}catch(error){report.status='failed';report.error=String(error);process.exitCode=1;console.error(error);}
finally{child.stdin.end();if(child.exitCode===null)child.kill();lines.close();report.stderr=stderr;report.finishedAt=new Date().toISOString();writeFileSync(join(directory,'report.json'),JSON.stringify(report,null,2));writeFileSync(join(candidate.directory,'installed-acceptance-latest.json'),JSON.stringify(report,null,2));}
