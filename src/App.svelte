<script lang="ts">
 import { onMount } from 'svelte';
 import { invoke } from '@tauri-apps/api/core';
 import { listen } from '@tauri-apps/api/event';
 import { open, save } from '@tauri-apps/plugin-dialog';
 import { readFile, writeTextFile } from '@tauri-apps/plugin-fs';
 import { FolderOpen, Files, Plus, Search, Settings2, History, ShieldAlert, Pencil, Trash2, Copy, ArrowLeft, Play, X, Check, AlertCircle, FileText, ChevronRight, Download, RotateCcw, FolderPlus, SlidersHorizontal, PanelLeftClose, PanelLeftOpen, Code2 } from 'lucide-svelte';
 import type { Profile, Rule, Kind, ScanProgress, ScanReport, FileResult } from './lib/types';
 const mobile=/Android/i.test(navigator.userAgent);
 const STORAGE='docinspector_profiles_v1';
 const HISTORY='docinspector_history_v1';
 const id=()=>Math.random().toString(36).slice(2)+Date.now().toString(36);
 let profiles:Profile[]=$state([]);
 let active:string=$state('');
 let page:'home'|'editor'|'results'|'settings'=$state('home');
 let editing:Profile|null=$state(null);
 let paths:string[]=$state([]);
 let result:ScanReport|null=$state(null);
 let progress:ScanProgress|null=$state(null);
 let busy=$state(false);
 let keyword=$state('');
 let newWord=$state('');
 let newKind:Kind=$state('forbidden');
 let tab:'all'|'issue'|'passed'|'unreadable'=$state('all');
 let selected:string=$state('');
 let notice=$state('');
 let history:{name:string;count:number;time:number}[]=$state([]);
 let sidebar=$state(!mobile);
 let cancelledMobile=false;
 let profile:Profile|undefined=$derived(profiles.find(p=>p.id===active));
 let filtered:FileResult[]=$derived(result?.files.filter(f=>tab==='all'||f.status===tab)??[]);
 let chosen:FileResult|undefined=$derived(result?.files.find(f=>f.path===selected));
 const example=():Profile=>({id:id(),name:'论文终稿检查',icon:'◆',color:'#9c92ed',created:Date.now(),rules:['TODO','待补充','占位符'].map(word=>({id:id(),word,kind:'forbidden',enabled:true}))});
 function persist(){localStorage.setItem(STORAGE,JSON.stringify(profiles));}
 onMount(()=>{
  try {profiles=JSON.parse(localStorage.getItem(STORAGE)||'null')||[example()];}catch{profiles=[example()];}
  active=profiles[0]?.id??''; persist();
  try {history=JSON.parse(localStorage.getItem(HISTORY)||'[]');}catch{}
  const cleanup=listen<ScanProgress>('scan-progress',e=>progress=e.payload);
  return ()=>{cleanup.then(fn=>fn());};
 });
 function create(){editing={id:id(),name:'新方案',icon:'◆',color:'#7faaca',created:Date.now(),rules:[]};newWord='';page='editor';}
 function edit(p:Profile){editing=structuredClone(p);newWord='';page='editor';}
 function saveProfile(){if(!editing)return;const name=editing.name.trim();if(!name){notice='请输入方案名称';return;}editing.name=name;const i=profiles.findIndex(p=>p.id===editing?.id);if(i>=0)profiles[i]=editing;else profiles=[...profiles,editing];profiles=[...profiles];active=editing.id;persist();page='home';}
 function remove(p:Profile){if(!confirm('删除方案“'+p.name+'”？'))return;profiles=profiles.filter(x=>x.id!==p.id);active=profiles[0]?.id??'';persist();}
 function duplicate(p:Profile){const clone=structuredClone(p);clone.id=id();clone.name+=' 副本';clone.rules=clone.rules.map(r=>({...r,id:id()}));profiles=[...profiles,clone];persist();}
 function addRule(){if(!editing)return;const terms=newWord.split(/[\n,，;；]+/).map(s=>s.trim()).filter(Boolean);for(const word of terms){if(!editing.rules.some(r=>r.word===word&&r.kind===newKind))editing.rules.push({id:id(),word,kind:newKind,enabled:true});}editing.rules=[...editing.rules];newWord='';}
 async function pick(directory:boolean){
  if(mobile && directory){notice='Android 暂不支持递归选择文件夹，请使用多文件选择';return;}
  try{
   const v=await open({directory,multiple:!directory,...(mobile?{pickerMode:'document' as const}:{})});
   if(v){const a=Array.isArray(v)?v:[v];paths=[...new Set([...paths,...a])];}
  }catch(e){notice=String(e);}
 }
 function simpleRules():Rule[]{
  return [...new Set(keyword.split(/[\n,，;；]+/).map(word=>word.trim()).filter(Boolean))]
    .map(word=>({id:id(),word,kind:'search' as Kind,enabled:true}));
 }
 async function start(){
  const rules=[...(profile?.rules||[]),...simpleRules()];
  if(!paths.length){notice='请先选择文件或文件夹';return;}
  if(!rules.some(r=>r.enabled&&r.word.trim())){notice='请添加关键词或选择有规则的方案';return;}
  busy=true; cancelledMobile=false; page='results'; result=null; progress=null; tab='all'; selected=''; notice='';
  try{
   if(mobile){
    const files:FileResult[]=[];
    for(const uri of paths){
     if(cancelledMobile)break;
     const name=decodeURIComponent(uri.split('/').pop()||'文档');
     try{
      const bytes=await readFile(uri);
      if(bytes.length>24*1024*1024)throw Error('文件超过 24 MB 移动端限制');
      const report=await invoke<FileResult>('scan_bytes',{name,bytes:Array.from(bytes),rules});
      files.push({...report,path:name});
     }catch(e){
      files.push({path:name,status:'unreadable',hits:[],missing:[],error:'无法读取所选文档：'+String(e)});
     }
     progress={scanned:files.length,total:paths.length,current:name};
     result={files:[...files],total:paths.length,scanned:files.length,cancelled:cancelledMobile};
    }
    if(!result)result={files,total:paths.length,scanned:files.length,cancelled:cancelledMobile};
   }else{
    result=await invoke<ScanReport>('scan_paths',{paths,rules});
   }
   selected=result.files[0]?.path??'';
   history=[{name:profile?.name||'快速检索',count:result.scanned,time:Date.now()},...history].slice(0,12);
   localStorage.setItem(HISTORY,JSON.stringify(history));
  }catch(e){notice=String(e);}finally{busy=false;}
 }
 async function cancel(){
  if(mobile){cancelledMobile=true;notice='将在当前文件结束后停止';return;}
  try{await invoke('cancel_scan');}catch(e){notice=String(e);}
 }
 async function exportCsv(){
  if(!result)return;
  try{
   if(mobile){
    const csv=await invoke<string>('export_csv_text',{report:result});
    try{
     const uri=await save({defaultPath:'docinspector-results.csv',filters:[{name:'CSV',extensions:['csv']}]});
     if(uri){await writeTextFile(uri,csv);notice='已保存 CSV';}
    }catch(e){
     try{await navigator.clipboard.writeText(csv);notice='无法写入所选位置，CSV 已复制到剪贴板';}
     catch{notice='保存失败：'+String(e);}
    }
   }else{
    const path=await save({defaultPath:'docinspector-results.csv',filters:[{name:'CSV',extensions:['csv']}]});
    if(path){await invoke('export_csv',{path,report:result});notice='已导出 CSV';}
   }
  }catch(e){notice=String(e);}
 }
 async function copyProfiles(){
  try{await navigator.clipboard.writeText(JSON.stringify({version:1,profiles},null,2));notice='方案配置已复制';}
  catch(e){notice='复制失败：'+String(e);}
 }
 function importProfiles(){
  const input=prompt('粘贴导出的方案 JSON');
  if(!input)return;
  try{
   const json=JSON.parse(input);
   if(!Array.isArray(json.profiles))throw Error('方案数据格式不正确');
   const valid=json.profiles.filter((p:any)=>
    p&&typeof p.name==='string'&&Array.isArray(p.rules)&&p.rules.every((r:any)=>
    typeof r.word==='string'&&['search','forbidden','required'].includes(r.kind)
   )).map((p:any)=>({...p,id:id(),rules:p.rules.map((r:Rule)=>({...r,id:id(),enabled:r.enabled!==false}))}));
   if(!valid.length)throw Error('没有有效方案');
   profiles=[...profiles,...valid];persist();notice='已导入 '+valid.length+' 个方案';
  }catch(e){notice='导入失败：'+String(e);}
 }
 const title=(p:string)=>p.split(/[\\/]/).pop()||p;
 const kindName=(k:Kind)=>({search:'检索',forbidden:'禁止',required:'必需'}[k]);
 const statusName=(s:string)=>({passed:'通过',issue:'问题',unreadable:'未检查'}[s]||s);
</script>
<svelte:head><title>DocInspector</title></svelte:head>
<div class="shell">
 <aside class:collapsed={!sidebar}>
  <div class="brand"><div class="brand-icon"><Search size={20}/></div>{#if sidebar}<span>DocInspector</span>{/if}<button class="icon push" title="切换侧栏" onclick={()=>sidebar=!sidebar}>{#if sidebar}<PanelLeftClose size={17}/>{:else}<PanelLeftOpen size={17}/>{/if}</button></div>
  <nav>
   <button class:current={page==='home'} onclick={()=>page='home'} title="工作台"><FolderOpen size={18}/>{#if sidebar}<span>工作台</span>{/if}</button>
   <button class:current={page==='results'} onclick={()=>page='results'} title="扫描结果"><FileText size={18}/>{#if sidebar}<span>扫描结果</span>{/if}</button>
  </nav>
  {#if sidebar}
   <div class="side-caption">我的方案 <button class="icon" title="新建方案" onclick={create}><Plus size={16}/></button></div>
   <div class="side-profiles">{#each profiles as p}<button class:active={active===p.id} onclick={()=>{active=p.id;page='home';}}><span class="dot" style:background={p.color}></span><span class="truncate">{p.name}</span></button>{/each}</div>
  {/if}
  <div class="side-bottom"><button class:current={page==='settings'} onclick={()=>page='settings'} title="设置"><Settings2 size={18}/>{#if sidebar}<span>设置</span>{/if}</button></div>
 </aside>
 <main>
  {#if page==='home'}
   <header><div><h1>工作台</h1><p>检索文件，检查内容</p></div><button class="subtle" onclick={()=>page='settings'} title="设置"><Settings2 size={19}/></button></header>
   <div class="content">
    <section class="source-section"><div class="section-head"><h2>扫描来源</h2>{#if paths.length}<button class="text-btn" onclick={()=>paths=[]}>清空</button>{/if}</div>
     <div class="sources">{#if !mobile}<button class="source-card" onclick={()=>pick(true)}><span class="source-icon"><FolderOpen size={23}/></span><strong>选择文件夹</strong><small>自动递归子目录</small></button>{/if}<button class="source-card" onclick={()=>pick(false)}><span class="source-icon"><Files size={23}/></span><strong>选择文件</strong><small>{mobile?"支持文档与代码多选":"支持多选和代码文件"}</small></button></div>
     {#if paths.length}<div class="path-list">{#each paths as p}<div class="path-chip"><FolderOpen size={14}/><span title={p}>{p}</span><button title="移除" onclick={()=>paths=paths.filter(x=>x!==p)}><X size={14}/></button></div>{/each}</div>{/if}
    </section>
    <section><div class="section-head"><h2>我的方案</h2><div class="profile-tools"><button class="text-btn" onclick={importProfiles}>导入</button><button class="text-btn" onclick={copyProfiles}>导出</button><button class="text-btn" onclick={create}><Plus size={16}/> 新建方案</button></div></div>
     <div class="profiles">{#each profiles as p}<article class:chosen={active===p.id} class="profile" role="button" tabindex="0" onclick={()=>active=p.id} onkeydown={e=>{if(e.key==='Enter')active=p.id}}><div class="profile-top"><div class="profile-symbol" style:background={p.color+'25'} style:color={p.color}><ShieldAlert size={22}/></div><div class="profile-actions"><button class="icon" title="编辑" onclick={e=>{e.stopPropagation();edit(p)}}><Pencil size={16}/></button><button class="icon" title="复制" onclick={e=>{e.stopPropagation();duplicate(p)}}><Copy size={16}/></button><button class="icon" title="删除" onclick={e=>{e.stopPropagation();remove(p)}}><Trash2 size={16}/></button></div></div><strong>{p.name}</strong><p class="preview">{p.rules.slice(0,4).map(r=>r.word).join(' · ')||'尚无规则'}</p><div class="profile-foot"><span>{p.rules.length} 条规则</span>{#if active===p.id}<span class="selected"><Check size={13}/> 已选择</span>{/if}</div></article>{/each}<button class="new-card" onclick={create}><Plus size={24}/><strong>新建方案</strong></button></div>
    </section>
    <section class="quick"><div class="section-head"><h2>临时检索</h2><span class="hint">可选</span></div><div class="search-input"><Search size={18}/><input bind:value={keyword} placeholder="输入关键词，多个词用逗号分隔"/></div></section>
    <div class="run-row"><div class="hint">{paths.length} 个来源 · {profile?.rules.filter(r=>r.enabled).length||0} 条方案规则</div><button class="primary" onclick={start}><Play size={16} fill="currentColor"/> 开始扫描 <ChevronRight size={17}/></button></div>
    {#if history.length}<section class="recent"><div class="section-head"><h2>最近任务</h2></div>{#each history.slice(0,4) as h}<div class="history-row"><History size={17}/><span>{h.name}</span><small>{h.count} 个文件</small><small>{new Date(h.time).toLocaleString('zh-CN')}</small></div>{/each}</section>{/if}
   </div>
  {:else if page==='editor'}
   <header><div class="header-with-back"><button class="icon" onclick={()=>page='home'}><ArrowLeft/></button><h1>{profiles.some(x=>x.id===editing?.id)?'编辑方案':'新建方案'}</h1></div><button class="primary" onclick={saveProfile}><Check size={16}/> 保存</button></header>
   {#if editing}<div class="content editor"><label class="field">方案名称<input bind:value={editing.name} placeholder="为方案命名"/></label><div class="section-head"><h2>关键词规则</h2><span class="hint">{editing.rules.length} 条</span></div><div class="rule-add"><select bind:value={newKind}><option value="forbidden">禁止出现</option><option value="required">必须包含</option><option value="search">仅检索</option></select><input bind:value={newWord} placeholder="添加关键词，支持逗号或换行分隔" onkeydown={e=>{if(e.key==='Enter')addRule()}}/><button class="primary" onclick={addRule}><Plus size={16}/> 添加</button></div><div class="rule-list">{#each editing.rules as rule,i}<div class="rule-item"><input type="checkbox" bind:checked={rule.enabled}/><span class:forbidden={rule.kind==='forbidden'} class:required={rule.kind==='required'} class="rule-kind">{kindName(rule.kind)}</span><span>{rule.word}</span><button class="icon push" title="移除" onclick={()=>{editing!.rules.splice(i,1);editing!.rules=[...editing!.rules]}}><X size={16}/></button></div>{:else}<div class="empty-small">添加关键词开始配置方案</div>{/each}</div><p class="hint">禁止词出现标记为问题；必须词缺失标记为问题；检索词仅记录位置。默认每个文件分别检查。</p></div>{/if}
  {:else if page==='results'}
   <header><div class="header-with-back"><button class="icon" title="返回工作台" onclick={()=>page='home'}><ArrowLeft/></button><div><h1>扫描结果</h1><p>{busy?'正在扫描':result?.cancelled?'扫描已取消':result?'扫描完成':'尚无扫描记录'}</p></div></div><div class="header-actions">{#if busy}<button class="outline" onclick={cancel}><X size={16}/> 取消</button>{:else if result}<button class="outline" onclick={exportCsv}><Download size={16}/> 导出 CSV</button><button class="outline" onclick={start}><RotateCcw size={16}/> 重新扫描</button>{/if}</div></header>
   {#if busy}<div class="progress"><div class="progress-label">已处理 {progress?.scanned||0} / {progress?.total||'…'} <span class="truncate">{progress?.current||'正在读取文件列表'}</span></div><div class="progress-bar"><div style:width={progress?.total?Math.min(100,100*progress.scanned/progress.total)+'%':'12%'}></div></div></div>{/if}
   {#if result}<div class="result-area"><div class="stats"><div><strong>{result.scanned}</strong><small>已检查</small></div><div><strong>{result.files.filter(f=>f.status==='passed').length}</strong><small>通过</small></div><div><strong>{result.files.filter(f=>f.status==='issue').length}</strong><small>有问题</small></div><div><strong>{result.files.filter(f=>f.status==='unreadable').length}</strong><small>未检查</small></div></div><div class="result-tabs">{#each [{k:'all',n:'全部'},{k:'issue',n:'问题'},{k:'passed',n:'通过'},{k:'unreadable',n:'未检查'}] as t}<button class:active={tab===t.k} onclick={()=>tab=t.k as typeof tab}>{t.n}</button>{/each}</div><div class="result-columns"><div class="file-results">{#each filtered as f}<button class:active={selected===f.path} class="file-result" onclick={()=>selected=f.path}><FileText size={17}/><span class="truncate"><strong>{title(f.path)}</strong><small title={f.path}>{f.path}</small></span><span class:issue={f.status==='issue'} class:unreadable={f.status==='unreadable'} class="status">{statusName(f.status)}</span></button>{:else}<div class="empty-small">没有符合筛选的文件</div>{/each}</div><div class="detail">{#if chosen}<div class="detail-head"><h2>{title(chosen.path)}</h2><p>{chosen.path}</p></div>{#if chosen.error}<div class="alert">{chosen.error}</div>{/if}{#each chosen.missing as word}<div class="hit missing"><div><AlertCircle size={16}/> 必须包含 · {word}</div><small>未找到</small></div>{/each}{#each chosen.hits as hit}<div class="hit"><div class:danger={hit.kind==='forbidden'} class="hit-label">{kindName(hit.kind)} · {hit.word}<small>{hit.location}</small></div><p>{#if hit.context.includes(hit.word)}{hit.context.slice(0,hit.context.indexOf(hit.word))}<mark>{hit.word}</mark>{hit.context.slice(hit.context.indexOf(hit.word)+hit.word.length)}{:else}{hit.context}{/if}</p></div>{:else}<div class="empty-small">没有命中关键词</div>{/each}{:else}<div class="empty-small">选择左侧文件查看命中内容</div>{/if}</div></div></div>{:else if !busy}<div class="empty-main"><FileText size={36}/><h2>尚无扫描结果</h2><button class="primary" onclick={()=>page='home'}>返回工作台</button></div>{/if}
  {:else if page==='settings'}
   <header><h1>设置</h1></header><div class="content settings"><h2>本地数据</h2><p>自定义方案与最近任务保存在此设备的应用数据中。扫描过程不会上传文件或修改原文。</p><div class="settings-row"><span>导出方案配置</span><button class="outline" onclick={async()=>{const path=await save({defaultPath:'docinspector-profiles.json',filters:[{name:'JSON',extensions:['json']}]});if(path){notice='当前版本支持在浏览器内保存配置；文件导入导出将在后续版本实现。';}}}>即将支持</button></div><h2>支持的格式</h2><p>PDF、DOCX、PPTX、XLSX、XLS、ODS、TXT、MD、CSV、LOG，以及 Python、Rust、JavaScript、TypeScript、C/C++、Java、Shell 和常见配置文件。暂不支持 OCR。</p></div>
  {/if}
  {#if notice}<div class="toast" role="status">{notice}<button title="关闭提示" onclick={()=>notice=''}><X size={15}/></button></div>{/if}
 </main>
</div>