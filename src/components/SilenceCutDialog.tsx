import { useEffect, useRef, useState } from "react";
import { Asset, FrameRate } from "../types/project";
import { SourceViewerDialog } from "./SourceViewerDialog";
import { editorApi } from "../services/editorApi";
import { SilenceConfig, SilenceJobReport, silenceKeepRanges } from "./silenceCuts";

export function SilenceCutDialog({ asset, projectId, revision, rate, onClose, onApply }: {
  asset: Asset; projectId: string; revision: number; rate: FrameRate; onClose: () => void;
  onLocate?: (asset: Asset, seconds: number) => void;
  onApply: (report: SilenceJobReport, selected: Set<number>) => Promise<void>;
}) {
  const [config, setConfig] = useState<SilenceConfig>({ thresholdDbfs: -40, minimumSilenceMs: 600, paddingMs: 150 });
  const [report, setReport] = useState<SilenceJobReport | null>(null);
  const [selected, setSelected] = useState(new Set<number>());
  const [cutFilter, setCutFilter] = useState<"all" | "cut" | "keep">("all");
  const cutFilterRef = useRef<HTMLSelectElement>(null);
  const [reviewFrame, setReviewFrame] = useState(0);
  const [viewerOpen, setViewerOpen] = useState(false);
  const [activeCut, setActiveCut] = useState<number | null>(null);
  const locate = (i: number) => {
    const cut = report?.analysis?.cuts[i]; if (!cut) return;
    const frame = Math.min(Math.max(0,(draft?.totalFrames??1)-1),Math.ceil(cut.startMicros/1e6*rate.numerator/rate.denominator-1e-8));
    setActiveCut(i); setReviewFrame(frame); setViewerOpen(true);
    document.getElementById(`silence-cut-${i}`)?.scrollIntoView({block:"nearest"});
  };
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [starting, setStarting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const job = useRef<string | null>(null);
  const alive = useRef(true);
  const running = starting || report?.state === "running";
  const stale = report && (report.projectId !== projectId || report.revision !== revision || report.sourcePath !== asset.path);
  const changed = report?.analysis && JSON.stringify(config) !== JSON.stringify(report.analysis.config);
  let draft: ReturnType<typeof silenceKeepRanges> | null = null;
  if (report?.analysis) draft = silenceKeepRanges(report.analysis, selected, rate);
  const visibleCutCount = report?.analysis?.cuts.filter((_, i) => cutFilter === "all" || selected.has(i) === (cutFilter === "cut")).length ?? 0;
  useEffect(() => {alive.current=true;return () => {alive.current=false;if(job.current)void editorApi.cancelSilenceAnalysis(job.current).catch(console.error);};}, []);
  useEffect(() => {
    if (report?.state !== "running") return;
    let disposed=false;let timer: ReturnType<typeof setTimeout>;let failures=0;
    const poll=async()=>{
      try {
        const next=await editorApi.silenceAnalysisStatus(report.jobId);
        if(disposed)return;failures=0;setReport(next);
        if(next.state === "completed")setSelected(new Set(next.analysis?.cuts.map((_, i)=>i)));
        if(next.state !== "running"){job.current=null;setCancelling(false);return;}
        timer=setTimeout(()=>void poll(),250);
      } catch(e){if(!disposed){setError(String(e));if(++failures>=3){
        void editorApi.cancelSilenceAnalysis(report.jobId).catch(cancelError=>{if(!disposed)setError(`Status and cancellation unavailable: ${String(cancelError)}`);});
        setReport(current=>current?{...current,state:"failed",message:"Job status unavailable. Cancellation requested; worker cleanup is unverified.",analysis:null}:current);setCancelling(false);
      }else timer=setTimeout(()=>void poll(),1000);}}
    };void poll();return()=>{disposed=true;clearTimeout(timer);};
  }, [report?.jobId, report?.state]);
  const analyze=async()=>{
    setStarting(true);setError(null);setReport(null);setSelected(new Set());setCutFilter("all");setReviewFrame(0);setActiveCut(null);
    try{const next=await editorApi.startSilenceAnalysis(asset.asset_id,config);if(!alive.current){await editorApi.cancelSilenceAnalysis(next.jobId);return;}job.current=next.jobId;setReport(next);}
    catch(e){if(alive.current)setError(String(e));}finally{if(alive.current)setStarting(false);}
  };
  return <><div className="dialog-backdrop"><section role="dialog" aria-modal="true" aria-labelledby="silence-title" className="modal-dialog" style={{width:"min(720px,94vw)",maxHeight:"85vh",overflow:"auto",padding:20}}>
    <h2 id="silence-title">Audio silence cuts</h2><p>{asset.name} · entire source, first audio stream</p>
    <p>Suggests low-amplitude intervals from audio, independently of transcript. Quiet speech can be marked too; review each cut. Clip gain, speed and fades are not analyzed.</p>
    <div style={{display:"flex",gap:12,flexWrap:"wrap"}}>
      <label>Threshold (dBFS) <input aria-label="Silence threshold dBFS" type="number" min={-80} max={-10} value={config.thresholdDbfs} disabled={running||busy} onChange={e=>setConfig({...config,thresholdDbfs:Number(e.target.value)})}/></label>
      <label>Minimum silence (ms) <input aria-label="Minimum silence ms" type="number" min={100} max={10000} value={config.minimumSilenceMs} disabled={running||busy} onChange={e=>setConfig({...config,minimumSilenceMs:Number(e.target.value)})}/></label>
      <label>Retain each edge (ms) <input aria-label="Silence padding ms" type="number" min={0} max={2000} value={config.paddingMs} disabled={running||busy} onChange={e=>setConfig({...config,paddingMs:Number(e.target.value)})}/></label>
    </div>
    <p>Mono/stereo · up to 30 minutes · no automatic timeline edits</p>
    <button className="btn-action" disabled={running||busy} onClick={()=>void analyze()}>{starting?"Starting…":"Analyze source audio"}</button>
    {running&&<button className="btn-action" disabled={cancelling||starting} onClick={()=>{if(report){setCancelling(true);void editorApi.cancelSilenceAnalysis(report.jobId).catch(e=>{setError(String(e));setCancelling(false);});}}}>{cancelling?"Cancelling…":"Cancel analysis"}</button>}
    {report&&<p role={report.state==="failed"?"alert":"status"}>{report.message}{running?` · ${Math.round(report.progress*100)}%`:""}</p>}
    {error&&<p role="alert">{error}</p>}
    {stale&&<p role="alert">Project or source changed. Analyze again before applying.</p>}
    {changed&&<p role="alert">Settings changed. Analyze again to update the draft.</p>}
    {report?.analysis&&<>
      <p aria-live="polite" aria-label="Silence cut selection summary">{selected.size} of {report.analysis.cuts.length} proposed cuts selected · removes {((draft?.removedFrames??0)*rate.denominator/rate.numerator).toFixed(2)} s · keeps {(((draft?.totalFrames??0)-(draft?.removedFrames??0))*rate.denominator/rate.numerator).toFixed(2)} s after frame alignment</p>
      {!report.analysis.cuts.length&&<p>No silence intervals meet these settings. The project is unchanged.</p>}
      <p>Quiet speech may be removed. Background noise or music may hide pauses; review the source before applying.</p>
      {draft && <div aria-label="Source review timeline">
        <strong>Source review · {(reviewFrame * rate.denominator / rate.numerator).toFixed(2)} s · frame {reviewFrame}</strong>
        <p>Keep / Cut shows the frame-aligned draft. Moving this marker does not play audio or change the edit.</p>
        <div style={{position:"relative",height:28,background:"var(--accent-warning)",margin:"8px 0"}} aria-label="Frame-aligned cut and keep map">
          {draft.ranges.map((range,i)=><span key={i} title={`Keep ${range.sourceInSeconds.toFixed(2)}–${range.sourceOutSeconds.toFixed(2)} s`} style={{position:"absolute",left:`${range.sourceInSeconds * rate.numerator / rate.denominator / draft.totalFrames * 100}%`,width:`${(range.sourceOutSeconds-range.sourceInSeconds)*rate.numerator/rate.denominator/draft.totalFrames*100}%`,height:"100%",background:"var(--accent-primary)"}} />)}
          <span style={{position:"absolute",left:`${reviewFrame/Math.max(1,draft.totalFrames)*100}%`,height:"100%",borderLeft:"2px solid var(--text-primary)"}} />
        </div>
        <label>Source review frame <input aria-label="Source review frame" type="range" min={0} max={Math.max(0,draft.totalFrames-1)} value={reviewFrame} disabled={busy} onChange={e=>setReviewFrame(Number(e.target.value))}/></label>
        <p>Keep: blue · Cut: amber. Detailed retained ranges are listed below.</p>
      </div>}
      <p aria-live="polite">{activeCut===null?"Choose a proposal to review.":`Reviewing proposal ${activeCut+1} · ${selected.has(activeCut)?"Cut":"Keep"}`}</p>
      <button type="button" className="btn-action" disabled={busy||!!stale||!!changed||activeCut===null||activeCut===0} onClick={()=>locate(activeCut!-1)}>Previous proposal</button>{" "}
      <button type="button" className="btn-action" disabled={busy||!!stale||!!changed||!report.analysis.cuts.length||activeCut===report.analysis.cuts.length-1} onClick={()=>locate(activeCut===null?0:activeCut+1)}>Next proposal</button>
      <button type="button" className="btn-action" disabled={busy || !report.analysis.cuts.length} onClick={()=>setSelected(new Set(report.analysis!.cuts.map((_,i)=>i)))}>Select all cuts</button>{" "}
      <button type="button" className="btn-action" disabled={busy || !selected.size} onClick={()=>setSelected(new Set())}>Clear selected cuts</button>{" "}
      <button type="button" className="btn-action" disabled={busy || !report.analysis.cuts.length} onClick={()=>setSelected(current=>new Set(report.analysis!.cuts.map((_,i)=>i).filter(i=>!current.has(i))))}>Invert cut selection</button>
      <label>Show proposals <select ref={cutFilterRef} aria-label="Filter silence proposals" value={cutFilter} onChange={e=>setCutFilter(e.target.value as typeof cutFilter)}><option value="all">All ({report.analysis.cuts.length})</option><option value="cut">Cut ({selected.size})</option><option value="keep">Keep ({report.analysis.cuts.length - selected.size})</option></select></label>
      {cutFilter !== "all" && <p>Bulk selection and previous/next review cover all proposals, including hidden ones.</p>}
      {activeCut !== null && cutFilter !== "all" && selected.has(activeCut) !== (cutFilter === "cut") && <p role="status">Reviewing proposal {activeCut + 1} is hidden by the current filter. <button type="button" onClick={()=>setCutFilter("all")}>Show reviewing proposal</button></p>}
      {cutFilter !== "all" && visibleCutCount === 0 && <p role="status">No {cutFilter === "cut" ? "Cut" : "Keep"} proposals. <button type="button" onClick={()=>setCutFilter("all")}>Show all proposals</button></p>}
      <ul>{report.analysis.cuts.map((cut,i)=>({cut,i})).filter(({i})=>cutFilter === "all" || selected.has(i) === (cutFilter === "cut")).map(({cut,i})=><li key={i} id={`silence-cut-${i}`} aria-current={activeCut===i?"true":undefined}><label><input type="checkbox" aria-label={`Remove silence interval ${i+1}`} checked={selected.has(i)} disabled={busy} onChange={e=>{const checked=e.currentTarget.checked;if(cutFilter !== "all" && checked !== (cutFilter === "cut")) cutFilterRef.current?.focus();setSelected(current=>{const next=new Set(current);if(checked)next.add(i);else next.delete(i);return next;});}}/><strong>{selected.has(i)?"Cut":"Keep"}</strong>{" · "}{(cut.startMicros/1e6).toFixed(2)}–{(cut.endMicros/1e6).toFixed(2)} s · {((cut.endMicros-cut.startMicros)/1e6).toFixed(2)} s proposed</label>{" "}<button type="button" className="btn-action" aria-label={`Review silence interval ${i+1}`} disabled={busy||!!stale||!!changed} onClick={()=>locate(i)}>Locate</button>{activeCut===i&&<span> · Reviewing</span>}</li>)}</ul>
      {draft && <div aria-label="Kept source ranges"><strong>Kept source ranges after frame alignment</strong>{draft.ranges.length ? <ol>{draft.ranges.map((range,i)=><li key={i}>{range.sourceInSeconds.toFixed(2)}–{range.sourceOutSeconds.toFixed(2)} s · {(range.sourceOutSeconds-range.sourceInSeconds).toFixed(2)} s</li>)}</ol> : <p>No source frames retained.</p>}</div>}
      {draft?.ranges.length===0&&<p role="alert">These cuts remove the entire source. Deselect a cut to retain content.</p>}
      <button className="btn-primary" disabled={busy||running||!!stale||!!changed||!draft?.removedFrames||!draft.ranges.length} onClick={async()=>{setBusy(true);setError(null);try{await onApply(report,new Set(selected));onClose();}catch(e){setError(String(e));}finally{setBusy(false);}}}>{busy?"Applying…":"Create silence rough cut"}</button>
      <p>Creates one new track at the playhead in one Undo step. Original clips and source remain unchanged. Video audio may overlap original tracks; review mute/solo before playback.</p>
    </>}
    <button className="btn-action" disabled={running||busy} onClick={onClose}>Close</button>
  </section></div>
    <SourceViewerDialog asset={viewerOpen ? asset : null} initialSeekSeconds={reviewFrame*rate.denominator/rate.numerator} onClose={()=>setViewerOpen(false)} onAddRange={()=>{}} reviewControls={activeCut!==null && <div aria-label="Silence proposal review controls">
      <p>Proposal {activeCut+1} · {selected.has(activeCut)?"Cut":"Keep"}</p>
      <button type="button" disabled={activeCut===0} onClick={()=>locate(activeCut-1)}>Previous proposal</button>
      <button type="button" disabled={activeCut===(report?.analysis?.cuts.length??0)-1} onClick={()=>locate(activeCut+1)}>Next proposal</button>
      <label><input type="checkbox" checked={selected.has(activeCut)} onChange={e=>setSelected(current=>{const next=new Set(current);if(e.target.checked)next.add(activeCut);else next.delete(activeCut);return next;})}/>Cut this proposal</label>
    </div>} />
  </>;
}
