import { spawn, execFileSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFileSync, writeFileSync, mkdirSync, copyFileSync, renameSync } from 'node:fs';
import { createHash, randomUUID } from 'node:crypto';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';
const candidate = JSON.parse(readFileSync('artifacts/candidates/20261116-090935605/installed-candidate.json','utf8'));
const binary=join(candidate.installation.directory,'tinixcut.exe');
const sha=file=>createHash('sha256').update(readFileSync(file)).digest('hex');
assert.equal(sha(binary),candidate.expectedInstalledBinary.sha256);
const directory=join(candidate.directory,'acceptance',new Date().toISOString().replaceAll(':','-'));mkdirSync(directory,{recursive:true});
const report={startedAt:new Date().toISOString(),binary,binarySha256:sha(binary),directory,scope:'Exact installed candidate CLI, no GUI/history/recovery/file-dialog claim',steps:[],limitations:['CLI exposes no Undo/Redo, recovery or collect; these require a qualified GUI/native route.','Project fixtures are disposable and edits use installed Command Service.']};
const child=spawn(binary,['api-session'],{windowsHide:true,stdio:['pipe','pipe','pipe'],cwd:candidate.installation.directory});
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
const cli=(...args)=>execFileSync(binary,args,{encoding:'utf8',windowsHide:true,timeout:45000,cwd:candidate.installation.directory});
const ffmpeg=join(candidate.installation.directory,'resources/bin/ffmpeg.exe'),ffprobe=join(candidate.installation.directory,'resources/bin/ffprobe.exe');
const sequences=new Map();
async function apply(file,operations,baseOverride){
 const p=read(file);const n=(sequences.get(p.project_id)??0)+1;
 const envelope={schema_version:p.schema_version,project_id:p.project_id,base_revision:baseOverride??p.revision,client_id:'installed-acceptance',client_epoch:1,sequence_number:n,transaction_id:`tx_${n}`,operations:operations.map((operation,i)=>({operation_id:`installed-acceptance:1:${n}:${i}`,operation}))};
 const result=await tool('tinixcut_apply',{projectPath:file,envelope},baseOverride!==undefined);
 if(baseOverride===undefined)sequences.set(p.project_id,n);
 return {envelope,result};
}
try {
 const project=join(directory,'cc0-draft.tcproj');await tool('tinixcut_project_create',{projectPath:project,name:'CC0 nature montage'});
 const files=[['a','misty.webm',47.18],['b','stream.webm',55.747],['c','grasshopper.webm',19.907]];
 const operations=files.map(([id,file,seconds])=>{const path=resolve('artifacts/background/draft-real',file);return {type:'AssetImport',payload:{asset_id:id,name:file,path,media_kind:'Video',duration:time(Math.floor(seconds*30)),file_size:readFileSync(path).length,dimensions:null}};});
 await apply(project,operations);const before=sha(project);
 const receipt=await tool('tinixcut_edit_plan_draft',{projectPath:project,goal:'A 15-second nature montage: misty river, grasshopper, flowing stream',mediaIds:['a','c','b'],targetDurationFrames:450});
 assert.equal(receipt.validation.valid,true);assert.equal(sha(project),before);
 const clips=receipt.plan.operations.filter(x=>x.operation.type==='ClipCreate'&&x.operation.payload.kind==='Video');assert.deepEqual(clips.map(x=>x.operation.payload.asset_id),['a','c','b']);
 report.scope='Exact installed CLI deterministic planner on three CC0 video files; read-only preview, no WebView or native picker';report.status='passed';report.order=clips.map(x=>x.operation.payload.asset_id);report.receipt=receipt;report.projectUnchanged=true;console.log('PASS: installed CLI real-media draft order');
}catch(e){report.status='failed';report.error=String(e);console.error(e);process.exitCode=1;}finally{child.stdin.end();child.kill();lines.close();report.finishedAt=new Date().toISOString();writeFileSync(join(candidate.directory,'installed-cc0-draft.json'),JSON.stringify(report,null,2));}
