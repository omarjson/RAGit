import { useEffect, useState } from "react";
import { FolderPlus, Pause, Play, Square, ListTree } from "lucide-react";
import { openFolderDialog } from "../lib/ipc";
import { useLibrary } from "../state/LibraryContext";
import { LEVEL_LABELS, type IndexLevel } from "../lib/ipc/constants";
import { ProgressBar } from "./ui/ProgressBar";
import { LevelSelect } from "./ui/LevelSelect";
import { FileList } from "./ui/FileList";

export function Indexing() {
  const { files, job, startIndex, pauseIndex, resumeIndex, cancelIndex, setScheduler, refreshFiles } = useLibrary();
  const [level, setLevel] = useState<IndexLevel>(4);
  const [schedulerOn, setSchedulerOn] = useState(false);
  const [status, setStatus] = useState("");

  useEffect(() => { refreshFiles(); }, [refreshFiles]);

  const indexFolder = async () => {
    const dir = await openFolderDialog();
    if (!dir) return;
    try { await startIndex(dir, undefined, level); } catch (e) { setStatus(`⚠ ${String(e)}`); }
  };

  const toggleScheduler = async () => {
    try {
      const msg = await setScheduler(undefined, !schedulerOn, 60);
      setSchedulerOn((v) => !v);
      setStatus(msg);
    } catch (e) { setStatus(`⚠ ${String(e)}`); }
  };

  const pct = job && job.total > 0 ? Math.round((job.processed / job.total) * 100) : 0;

  return (
    <div className="p-6">
      <h1 className="mb-1 text-2xl font-semibold text-primary">Indexing</h1>
      <p className="mb-6 text-sm text-muted">Background indexing with 5 depth levels. Start, pause, resume, or cancel — progress is saved.</p>

      <div className="mb-6 flex flex-wrap items-center gap-3">
        <button onClick={indexFolder} disabled={!!job}
          className="btn-ghost text-sm">
          <FolderPlus size={16} /> Index Folder
        </button>

        <div className="flex items-center gap-2">
          <label className="text-xs text-muted">Depth level</label>
          <LevelSelect value={level} onChange={setLevel} />
        </div>

        <label className="flex items-center gap-2 text-sm text-[#A5ABB8]">
          <input type="checkbox" checked={schedulerOn} onChange={toggleScheduler}
            className="accent-brand-fg" /> Auto-reindex
        </label>
      </div>

      {job && (
        <div className="mb-6 space-y-2 rounded-sm border border-[#2A2D45] bg-[#141626] p-4">
          <div className="flex items-center gap-2 text-sm text-[#D0D2E0]">
            <ListTree size={16} className="text-brand-fg" />
            <span>{job.currentFile || "Starting…"}</span>
          </div>
          <div className="font-mono text-[11px] text-muted">
            {LEVEL_LABELS[job.level as IndexLevel]} · {job.status} · {job.processed}/{job.total}
          </div>
          <ProgressBar percent={pct} />
          <div className="truncate font-mono text-[11px] text-muted">{job.message}</div>
          <div className="flex gap-2">
            <button onClick={() => pauseIndex()}
              className="btn-ghost flex-1 text-xs py-1">
              <Pause size={11} /> Pause
            </button>
            <button onClick={() => resumeIndex()}
              className="btn-ghost flex-1 text-xs py-1">
              <Play size={11} /> Resume
            </button>
            <button onClick={() => cancelIndex()}
              className="btn-ghost flex-1 text-xs py-1">
              <Square size={11} /> Cancel
            </button>
          </div>
        </div>
      )}

      {status && (
        <div className="mb-6 rounded-sm border border-[#2A2D45] bg-[#141626] px-3 py-2 font-mono text-[11px] text-muted">{status}</div>
      )}

      <div className="rounded-sm border border-[#2A2D45] bg-[#141626] p-4">
        <div className="mb-3 flex items-center gap-2 text-sm text-[#A5ABB8]">
          <ListTree size={16} /> Indexed files ({files.length})
        </div>
        <FileList files={files} />
      </div>
    </div>
  );
}
