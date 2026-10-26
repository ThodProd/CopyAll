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

async function contextAction(method){
 const install=$('context-install'),remove=$('context-remove');install.disabled=remove.disabled=true;
 try{const state=await api('worker.context.'+method);$('context-status').textContent=state.installed?'Пункт CopyAll установлен для текущего пользователя.':'Пункт CopyAll не установлен.';install.disabled=state.installed;remove.disabled=!state.installed;}
 catch(e){$('context-status').textContent=e.message;install.disabled=remove.disabled=false;}
}
$('context-install').onclick=()=>contextAction('install');$('context-remove').onclick=()=>contextAction('remove');void contextAction('status');

document.documentElement.dataset.theme=new URLSearchParams(location.search).get('theme')||'light';
