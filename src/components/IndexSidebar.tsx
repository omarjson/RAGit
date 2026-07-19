import { FolderPlus, Pause, Play, Square, Database } from "lucide-react";
import { openFolderDialog } from "../lib/ipc";
import { useEngine } from "../state/EngineContext";
import { useLibrary } from "../state/LibraryContext";
import { LEVEL_LABELS, type IndexLevel } from "../lib/ipc/constants";
import { ProgressBar } from "./ui/ProgressBar";
import { LevelSelect } from "./ui/LevelSelect";
import { FileListCompact } from "./ui/FileList";
import { useState } from "react";

export function IndexSidebar({ level, onLevelChange, ragMode, onRagModeChange, rerank, onRerankChange }:
  { level: IndexLevel; onLevelChange: (v: IndexLevel) => void; ragMode: boolean; onRagModeChange: (v: boolean) => void; rerank: boolean; onRerankChange: (v: boolean) => void }
) {
  const { status: engine } = useEngine();
  const { files, job, startIndex, pauseIndex, resumeIndex, cancelIndex, setScheduler, loading } = useLibrary();
  const [schedulerOn, setSchedulerOn] = useState(false);
  const [status, setStatus] = useState("");

  const engineUp = engine?.running ?? false;

  const indexFolder = async () => {
    const dir = await openFolderDialog();
    if (!dir) return;
    try {
      await startIndex(dir, undefined, level);
    } catch (e) { setStatus(`⚠ ${String(e)}`); }
  };

  const pct = job && job.total > 0 ? Math.round((job.processed / job.total) * 100) : 0;

  return (
    <div className="flex w-72 flex-col gap-3 border-r border-[#2A2D45] p-4">
      <button onClick={indexFolder} disabled={!!job || loading}
        className="btn-ghost w-full justify-center text-sm">
        <FolderPlus size={16} /> Index Folder
      </button>

      <div>
        <label className="text-xs text-muted">Depth level</label>
        <div className="mt-1"><LevelSelect value={level} onChange={onLevelChange} /></div>
      </div>

      {job && (
        <div className="space-y-2 rounded-sm border border-[#2A2D45] p-2">
          <div className="font-mono text-[11px] text-[#A5ABB8]">{job.status} · {job.processed}/{job.total} · {LEVEL_LABELS[job.level as IndexLevel]}</div>
          <ProgressBar percent={pct} />
          <div className="truncate font-mono text-[11px] text-muted">{job.message}</div>
          <div className="flex gap-2">
            <button onClick={() => pauseIndex()} className="btn-ghost flex-1 text-[11px] py-1"><Pause size={11} className="inline" /> Pause</button>
            <button onClick={() => resumeIndex()} className="btn-ghost flex-1 text-[11px] py-1"><Play size={11} className="inline" /> Resume</button>
            <button onClick={() => cancelIndex()} className="btn-ghost flex-1 text-[11px] py-1"><Square size={11} className="inline" /> Cancel</button>
          </div>
        </div>
      )}

      <label className="flex items-center gap-2 text-sm text-[#A5ABB8]">
        <input type="checkbox" checked={ragMode} onChange={(e) => onRagModeChange(e.target.checked)} className="accent-brand-fg" /> RAG mode
      </label>
      <label className="flex items-center gap-2 text-sm text-[#A5ABB8]">
        <input type="checkbox" checked={schedulerOn} onChange={async () => {
          try {
            const msg = await setScheduler(undefined, !schedulerOn, 60);
            setSchedulerOn((v) => !v);
            setStatus(msg);
          } catch (e) { setStatus(`⚠ ${String(e)}`); }
        }} className="accent-brand-fg" /> Auto-reindex
      </label>
      <label className="flex items-center gap-2 text-sm text-[#A5ABB8]">
        <input type="checkbox" checked={rerank} onChange={(e) => onRerankChange(e.target.checked)} className="accent-brand-fg" /> Rerank chunks
      </label>

      <div className="flex items-center gap-2 text-sm text-[#A5ABB8]">
        <Database size={16} />
        <span className={engineUp ? "text-accent" : "text-muted"}>
          {engineUp ? "● engine live" : "○ engine off"}
        </span>
      </div>

      <div className="flex-1 overflow-auto">
        <FileListCompact files={files} />
      </div>

      {status && <div className="font-mono text-[11px] text-muted">{status}</div>}
    </div>
  );
}
