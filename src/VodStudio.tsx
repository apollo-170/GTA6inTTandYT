import React,{useState} from 'react';
import * as api from './api';
import './vod.css';
type Props={accounts:api.ConnectedAccount[]};
const fmt=(v:number)=>{const h=Math.floor(v/3600),m=Math.floor(v%3600/60),s=Math.floor(v%60);return [h,m,s].map(x=>String(x).padStart(2,'0')).join(':')};
export default function VodStudio({accounts}:Props){
 const [vod,setVod]=useState<api.VodInfo|null>(null);
 const [start,setStart]=useState(0),[end,setEnd]=useState(60),[seek,setSeek]=useState(0);
 const [frame,setFrame]=useState(''),[file,setFile]=useState(''),[mode,setMode]=useState<'vertical'|'wide'>('vertical');
 const [title,setTitle]=useState('ГТА 6 — лучшие моменты стрима'),[privacy,setPrivacy]=useState('private');
 const [chosen,setChosen]=useState<string[]>([]),[agreed,setAgreed]=useState(false),[busy,setBusy]=useState('');
 const [notice,setNotice]=useState(''),[uploads,setUploads]=useState<string[]>([]);
 const yt=accounts.filter(a=>a.platform==='youtube');
 const run=async(label:string,fn:()=>Promise<void>)=>{setBusy(label);setNotice('');try{await fn()}catch(e){setNotice(String(e))}finally{setBusy('')}};
 const load=()=>run('load',async()=>{const path=await api.pickFile([{name:'Запись стрима',extensions:['mp4','mkv','mov','webm']}]);if(!path)return;const info=await api.vodInfo(path);setVod(info);setStart(0);setSeek(0);setEnd(Math.min(60,info.duration));setFile('');setUploads([]);setFrame(await api.vodFrame(path,0));});
 const preview=()=>run('preview',async()=>{if(vod)setFrame(await api.vodFrame(vod.path,Math.min(seek,Math.max(0,vod.duration-0.01))))});
 const clip=()=>run('cut',async()=>{if(!vod)return;const output=await api.vodCut(vod.path,start,end,mode==='vertical');setFile(output);setNotice('Готовый фрагмент: '+output)});
 const publish=(whole:boolean)=>run('publish',async()=>{
  if(!vod||!agreed)throw Error('Подтверди права на видео и публикацию');
  const path=whole?vod.path:file;if(!path)throw Error('Сначала сделай фрагмент');
  if(chosen.length===0)throw Error('Выбери YouTube аккаунт');
  const results=await api.vodYouTubeUpload(path,chosen,title,privacy,true);
  setUploads(results);
 });
 return <div className="vod-page">
  <div className="vod-hero"><span>VOD EDITOR · FULL STREAM + CLIPS</span><h1>Записи стримов</h1><p>Просматривай полную запись по кадрам, выбирай фрагменты на таймлайне и загружай на YouTube. Всё обрабатывается локально, без платного сервера.</p></div>
  <div className="vod-layout">
   <section className="vod-panel"><h2>01 / Исходный стрим</h2><button className="vod-primary" disabled={!!busy} onClick={load}>{busy==='load'?'Открываю…':'Выбрать запись стрима'}</button>
   {vod?<><div className="vod-file"><b>{vod.name}</b><small>{fmt(vod.duration)} · {(vod.size/1024**3).toFixed(2)} ГБ</small></div>
   <div className="vod-preview">{frame?<img src={frame} alt="Кадр записи стрима"/>:<span>Кадр ещё не загружен</span>}</div>
   <div className="vod-timing"><span>Позиция {fmt(seek)}</span><strong>{fmt(vod.duration)}</strong></div>
   <input aria-label="Позиция просмотра" type="range" min={0} max={Math.max(0.1,vod.duration-0.01)} step={1} value={seek} onChange={e=>setSeek(Number(e.target.value))}/>
   <button className="vod-secondary" disabled={!!busy} onClick={preview}>{busy==='preview'?'Загружаю…':'Показать кадр на выбранной секунде'}</button>
   <p className="vod-hint">Просмотр по кадрам не требует копирования всей записи. Выбирай секунду ползунком.</p>
   </>:<p className="vod-hint">Выбери MP4, MKV, MOV или WEBM, записанный тобой или разрешённый владельцем.</p>}
   </section>
   <section className="vod-panel"><h2>02 / Нарезка</h2>
   <label>Начало, секунды<input type="number" min={0} max={vod?.duration??0} step={0.1} value={start} onChange={e=>setStart(Number(e.target.value))}/></label>
   <label>Конец, секунды<input type="number" min={0} max={vod?.duration??0} step={0.1} value={end} onChange={e=>setEnd(Number(e.target.value))}/></label>
   <div className="vod-presets"><button className={mode==='vertical'?'picked':''} onClick={()=>setMode('vertical')}>9:16 Shorts · до 3 мин</button><button className={mode==='wide'?'picked':''} onClick={()=>setMode('wide')}>16:9 YouTube</button></div>
   <div className="vod-timing"><span>Выбранный фрагмент</span><strong>{fmt(Math.max(0,end-start))}</strong></div>
   <button className="vod-primary" disabled={!vod||!!busy||end<=start||(mode==='vertical'&&end-start>180)} onClick={clip}>{busy==='cut'?'Экспортируется…':'Вырезать и сохранить MP4'}</button>
   {file&&<div className="vod-file"><b>Готово к публикации</b><small title={file}>{file}</small></div>}
   <p className="vod-hint">Вертикальный монтаж: размытый фон и исходное видео по центру. Для горизонтального ролика сохраняется формат 16:9.</p>
   </section>
   <section className="vod-panel"><h2>03 / Загрузка на YouTube</h2>
   <label>Название<input maxLength={100} value={title} onChange={e=>setTitle(e.target.value)}/></label>
   <label>Видимость<select value={privacy} onChange={e=>setPrivacy(e.target.value)}><option value="private">Приватное</option><option value="unlisted">По ссылке</option><option value="public">Публичное</option></select></label>
   <div className="vod-accounts"><b>Каналы YouTube</b>{yt.length?yt.map(a=><label key={a.id}><input type="checkbox" checked={chosen.includes(a.id)} onChange={e=>setChosen(old=>e.target.checked?[...old,a.id]:old.filter(x=>x!==a.id))}/>{a.name}</label>):<p className="vod-hint">Сначала подключи YouTube во вкладке «Публикация».</p>}</div>
   <label className="vod-consent"><input type="checkbox" checked={agreed} onChange={e=>setAgreed(e.target.checked)}/> Подтверждаю права на этот материал и публикацию в выбранных каналах.</label>
   <button className="vod-primary" disabled={!vod||!!busy||!agreed||!chosen.length} onClick={()=>publish(true)}>{busy==='publish'?'Загружаю…':'Загрузить весь стрим на YouTube'}</button>
   <button className="vod-secondary" disabled={!file||!!busy||!agreed||!chosen.length} onClick={()=>publish(false)}>Загрузить нарезку на YouTube</button>
   <p className="vod-hint">Полная загрузка принимает MP4 и отправляет файл частями без чтения его целиком в RAM. Приложение должно оставаться открытым.</p>
   {uploads.map((x,i)=><p key={i} className="vod-result">{x}</p>)}
   </section>
  </div>
  {notice&&<div role="status" className="vod-notice">{notice}</div>}
 </div>;
}
