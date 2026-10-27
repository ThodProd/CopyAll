'use strict';
const $=id=>document.getElementById(id), api=(method,params)=>window.thodHub.call(method,params);
const status=text=>$('status').textContent=text;
let settings={path:'',mode:'tree',includeHidden:false};
async function persist(){settings={path:$('path').value,mode:$('mode').value,includeHidden:$('hidden').checked};await api('settings.set',settings);}
$('choose').onclick=async()=>{try{const path=await api('dialog.folder');if(path){$('path').value=path;$('scan').disabled=false;await persist();}}catch(e){status(e.message);}};
$('scan').onclick=async()=>{const button=$('scan');button.disabled=true;status('Читаем папку…');try{await persist();const result=await api('worker.scan',settings);$('result').value=result.text;$('copy').disabled=false;$('save').disabled=false;status(`Файлов: ${result.files} · Папок: ${result.folders} · Пропущено: ${result.skipped}`);}catch(e){status(e.message);}finally{button.disabled=false;}};
$('copy').onclick=async()=>{try{await api('clipboard.write',{text:$('result').value});status('Список скопирован.');}catch(e){status(e.message);}};
$('save').onclick=async()=>{try{const bytes=new TextEncoder().encode($('result').value);let binary='';for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));const r=await api('files.export',{name:'Структура.txt',base64:btoa(binary)});if(!r.canceled)status('Файл сохранён.');}catch(e){status(e.message);}};
(async()=>{try{settings=await api('settings.get')||settings;$('path').value=settings.path||'';$('mode').value=settings.mode||'tree';$('hidden').checked=!!settings.includeHidden;$('scan').disabled=!settings.path;}catch(e){status(e.message);}})();

let contextState=null;
function renderContext(state){
 contextState=state;
 $('context-status').textContent=state.modernInstalled?'CopyAll включён в первом меню Windows 11 для папок и файлов.':state.installed?'CopyAll включён в классическом меню. Для первого меню Windows 11 установите расширение.':'Пункт CopyAll не установлен.';
 $('context-install').disabled=state.installed&&(!state.modernSupported||state.modernInstalled);
 $('context-remove').disabled=!state.installed&&!state.modernInstalled;
 $('context-trust').hidden=!state.modernSupported||state.trusted;
}
async function contextAction(method){
 const install=$('context-install'),remove=$('context-remove');install.disabled=remove.disabled=true;
 try{
  if(method==='install'){
   if(contextState?.modernSupported&&!contextState.trusted&&!$('trust-certificate').checked)throw new Error('Подтвердите доверие сертификату расширения перед установкой.');
   $('context-status').textContent='Устанавливаем расширение Проводника… При запросе подтвердите UAC.';
   await api('worker.context.install.start',{trustCertificate:$('trust-certificate').checked});
   for(;;){await new Promise(r=>setTimeout(r,700));const job=await api('worker.context.install.status');if(job.running)continue;if(job.error)throw new Error(job.error);renderContext(job.state);break;}
  }else renderContext(await api('worker.context.'+method));
 }catch(e){$('context-status').textContent=e.message;install.disabled=remove.disabled=false;}
}
$('context-install').onclick=()=>contextAction('install');$('context-remove').onclick=()=>contextAction('remove');void contextAction('status');
document.documentElement.dataset.theme=new URLSearchParams(location.search).get('theme')||'light';
