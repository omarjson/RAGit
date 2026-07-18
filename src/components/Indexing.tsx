import { useEffect, useState } from "react";
import { FolderPlus, Pause, Play, Square, ListTree } from "lucide-react";
import {
  indexLibrary,
  pauseIndex,
  resumeIndex,
  cancelIndex,
  listIndexedFiles,
  setScheduler,
  openFolderDialog,
  IndexProgress,
  IndexedFile,
} from "../lib/ipc";

const LIB = "default";
const LEVEL_LABELS = ["", "L1 raw", "L2 structure", "L3 summaries", "L4 dense", "L5 rerank"];

export function Indexing() {
  const [level, setLevel] = useState(4);
  const [jobActive, setJobActive] = useState(false);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [files, setFiles] = useState<IndexedFile[]>([]);
  const [schedulerOn, setSchedulerOn] = useState(false);
  const [status, setStatus] = useState("");

  useEffect(() => {
    listIndexedFiles(LIB).then(setFiles).catch(() => {});
  }, []);

  const refreshFiles = () => listIndexedFiles(LIB).then(setFiles).catch(() => {});

  const indexFolder = async () => {
    const dir = await openFolderDialog();
    if (!dir) return;
    setJobActive(true);
    setProgress(null);
    setStatus("");
    try {
      const msg = await indexLibrary(dir, LIB, level, (p) => {
        setProgress(p);
        if (p.status === "done" || p.status === "canceled" || p.status === "error") {
          setJobActive(false);
          refreshFiles();
        }
      });
      setStatus(msg);
      setJobActive(false);
      refreshFiles();
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
      setJobActive(false);
    }
  };

  const toggleScheduler = async () => {
    try {
      const msg = await setScheduler(LIB, !schedulerOn, 60);
      setSchedulerOn((v) => !v);
      setStatus(msg);
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
    }
  };

  const pct = progress && progress.total > 0
    ? Math.round((progress.processed / progress.total) * 100)
    : 0;

  return (
    <div className="p-6">
      <h1 className="mb-1 text-2xl font-semibold">Indexing</h1>
      <p className="mb-6 text-sm text-zinc-400">
        Background indexing with 5 depth levels. Start, pause, resume, or cancel — progress is saved.
      </p>

      <div className="mb-6 flex flex-wrap items-center gap-3">
        <button
          onClick={indexFolder}
          disabled={jobActive}
          className="flex items-center justify-center gap-2 rounded-md border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-800 disabled:opacity-40"
        >
          <FolderPlus size={16} /> Index Folder
        </button>

        <div>
          <label className="mr-2 text-xs text-zinc-400">Depth level</label>
          <select
            value={level}
            onChange={(e) => setLevel(Number(e.target.value))}
            className="rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-sm text-zinc-100"
          >
            {[1, 2, 3, 4, 5].map((l) => (
              <option key={l} value={l}>
                {LEVEL_LABELS[l]}
              </option>
            ))}
          </select>
        </div>

        <label className="flex items-center gap-2 text-sm text-zinc-300">
          <input type="checkbox" checked={schedulerOn} onChange={toggleScheduler} />
          Auto-reindex (scheduler)
        </label>
      </div>

      {jobActive && (
        <div className="mb-6 space-y-2 rounded-lg border border-zinc-700 bg-zinc-900/40 p-4">
          <div className="flex items-center gap-2 text-sm text-zinc-200">
            <ListTree size={16} className="text-brand-fg" />
            <span>{progress?.currentFile || "Starting…"}</span>
          </div>
          <div className="text-xs text-zinc-400">
            {LEVEL_LABELS[progress?.level ?? 0]} · {progress?.status} · {progress?.processed}/{progress?.total}
          </div>
          <div className="h-2 w-full overflow-hidden rounded bg-zinc-800">
            <div className="h-full bg-brand-fg transition-all" style={{ width: `${pct}%` }} />
          </div>
          <div className="truncate text-[11px] text-zinc-500">{progress?.message}</div>
          <div className="flex gap-2">
            <button
              onClick={() => pauseIndex(LIB)}
              className="flex flex-1 items-center justify-center gap-1 rounded bg-zinc-800 px-2 py-1 text-xs text-zinc-200 hover:bg-zinc-700"
            >
              <Pause size={12} /> Pause
            </button>
            <button
              onClick={() => resumeIndex(LIB)}
              className="flex flex-1 items-center justify-center gap-1 rounded bg-zinc-800 px-2 py-1 text-xs text-zinc-200 hover:bg-zinc-700"
            >
              <Play size={12} /> Resume
            </button>
            <button
              onClick={() => cancelIndex(LIB)}
              className="flex flex-1 items-center justify-center gap-1 rounded bg-zinc-800 px-2 py-1 text-xs text-zinc-200 hover:bg-zinc-700"
            >
              <Square size={12} /> Cancel
            </button>
          </div>
        </div>
      )}

      {status && (
        <div className="mb-6 rounded-md border border-zinc-800 bg-zinc-900/40 px-3 py-2 text-xs text-zinc-400">
          {status}
        </div>
      )}

      <div className="rounded-lg border border-zinc-800 bg-zinc-900/30 p-4">
        <div className="mb-3 flex items-center gap-2 text-sm text-zinc-300">
          <ListTree size={16} /> Indexed files ({files.length})
        </div>
        {files.length === 0 ? (
          <div className="text-sm text-zinc-500">No indexed files yet. Use “Index Folder” to begin.</div>
        ) : (
          <div className="space-y-1">
            {files.map((f) => (
              <div
                key={f.id}
                className="flex items-center justify-between gap-3 rounded border border-zinc-800 px-2 py-1.5 text-[11px]"
              >
                <span className="truncate text-zinc-200">{f.fileName}</span>
                <span className="shrink-0 text-zinc-500">
                  <span
                    className={
                      f.status === "done"
                        ? "text-emerald-400"
                        : f.status === "error"
                        ? "text-red-400"
                        : "text-zinc-500"
                    }
                  >
                    {f.status}
                  </span>{" "}
                  · L{f.level} · {f.chunks} chunks
                </span>
                {f.error && <span className="shrink-0 text-red-400">{f.error}</span>}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
