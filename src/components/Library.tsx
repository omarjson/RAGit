import { useEffect, useState } from "react";
import { FolderPlus, Upload, Download, FileWarning } from "lucide-react";
import {
  indexLibrary,
  listIndexedFiles,
  exportLibrary,
  importLibrary,
  openFolderDialog,
  IndexProgress,
  IndexedFile,
} from "../lib/ipc";

const LEVEL_LABELS = ["", "L1 raw", "L2 structure", "L3 summaries", "L4 dense", "L5 rerank"];
const LIB_ID = "default";

interface Job {
  path: string;
  progress: IndexProgress | null;
}

export function Library() {
  const [level, setLevel] = useState(4);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [files, setFiles] = useState<IndexedFile[]>([]);
  const [status, setStatus] = useState("");
  const [dragging, setDragging] = useState(false);

  const refreshFiles = () =>
    listIndexedFiles(LIB_ID).then(setFiles).catch(() => {});

  useEffect(() => {
    refreshFiles();
  }, []);

  const runIndex = async (path: string) => {
    const jobId = path;
    setJobs((prev) => [...prev, { path: jobId, progress: null }]);
    try {
      const msg = await indexLibrary(path, LIB_ID, level, (p) => {
        setJobs((prev) =>
          prev.map((j) => (j.path === jobId ? { ...j, progress: p } : j))
        );
        if (p.status === "done" || p.status === "canceled" || p.status === "error") {
          refreshFiles();
        }
      });
      setStatus(msg);
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
      refreshFiles();
    } finally {
      setJobs((prev) => prev.filter((j) => j.path !== jobId));
    }
  };

  const indexFolder = async () => {
    const dir = await openFolderDialog();
    if (!dir) return;
    await runIndex(dir);
  };

  const onDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
    const list = e.dataTransfer.files;
    for (let i = 0; i < list.length; i++) {
      const item = list[i] as any;
      const path: string | undefined =
        item.path ?? item.webkitRelativePath ?? item.name;
      if (path) await runIndex(path);
    }
  };

  const doExport = async () => {
    const p = await import("@tauri-apps/plugin-dialog").then((m) =>
      m.save({
        defaultPath: `${LIB_ID}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      })
    );
    if (!p) return;
    try {
      setStatus(await exportLibrary(p as string, LIB_ID));
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
    }
  };

  const doImport = async () => {
    const f = await import("@tauri-apps/plugin-dialog").then((m) =>
      m.open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      })
    );
    if (!f) return;
    try {
      setStatus(await importLibrary(f as string, LIB_ID));
      refreshFiles();
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
    }
  };

  return (
    <div className="p-6">
      <h1 className="mb-1 text-2xl font-semibold">Library</h1>
      <p className="mb-6 text-sm text-zinc-400">
        Drag &amp; drop files (text, documents, images, video, audio) or a folder to build your knowledge base.
      </p>

      <div className="mb-4 flex gap-3">
        <button
          onClick={indexFolder}
          className="flex items-center gap-2 rounded-md border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-800"
        >
          <FolderPlus size={16} /> Select Folder
        </button>
        <div className="flex items-center gap-2">
          <label className="text-xs text-zinc-400">Depth level</label>
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
        <div className="ml-auto flex gap-2">
          <button
            onClick={doExport}
            className="flex items-center gap-1 rounded-md border border-zinc-700 px-3 py-1 text-xs text-zinc-300 hover:bg-zinc-800"
          >
            <Download size={14} /> Export
          </button>
          <button
            onClick={doImport}
            className="flex items-center gap-1 rounded-md border border-zinc-700 px-3 py-1 text-xs text-zinc-300 hover:bg-zinc-800"
          >
            <Upload size={14} /> Import
          </button>
        </div>
      </div>

      <div
        onDragOver={(e) => {
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={onDrop}
        className={`mb-6 rounded-lg border border-dashed p-10 text-center transition-colors ${
          dragging
            ? "border-brand-fg bg-brand-fg/5 text-zinc-300"
            : "border-zinc-800 text-zinc-500"
        }`}
      >
        Drop files or folders here (currently using {LEVEL_LABELS[level]})
      </div>

      {jobs.length > 0 && (
        <div className="mb-6 space-y-2">
          {jobs.map((j) => {
            const p = j.progress;
            const pct =
              p && p.total > 0
                ? Math.round((p.processed / p.total) * 100)
                : 0;
            return (
              <div key={j.path} className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-3">
                <div className="mb-1 flex items-center justify-between text-xs text-zinc-300">
                  <span className="truncate">{p?.currentFile || j.path}</span>
                  <span className="text-zinc-500">
                    {p?.status} · {p?.processed}/{p?.total} · {LEVEL_LABELS[p?.level ?? 0]}
                  </span>
                </div>
                <div className="h-2 w-full overflow-hidden rounded bg-zinc-800">
                  <div
                    className="h-full bg-brand-fg transition-all"
                    style={{ width: `${pct}%` }}
                  />
                </div>
                {p?.message && (
                  <div className="mt-1 truncate text-[11px] text-zinc-500">{p.message}</div>
                )}
              </div>
            );
          })}
        </div>
      )}

      <div className="rounded-lg border border-zinc-800 bg-zinc-900/30 p-4">
        <div className="mb-2 text-sm text-zinc-400">Indexed files ({files.length})</div>
        {files.length === 0 && (
          <div className="py-6 text-center text-xs text-zinc-600">No files indexed yet.</div>
        )}
        <div className="space-y-1">
          {files.map((f) => (
            <div
              key={f.id}
              className="flex items-center justify-between rounded-md border border-zinc-800 bg-zinc-900/40 px-3 py-1.5 text-sm"
            >
              <span className="truncate text-zinc-200">{f.fileName}</span>
              <div className="flex items-center gap-2 text-[11px]">
                <span
                  className={
                    f.status === "done"
                      ? "text-emerald-400"
                      : f.status === "error"
                      ? "text-red-400"
                      : "text-zinc-500"
                  }
                >
                  {f.status} · L{f.level} · {f.chunks} chunks
                </span>
                {f.error && (
                  <span className="flex items-center gap-1 text-red-400">
                    <FileWarning size={12} /> {f.error}
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>

      {status && <div className="mt-4 text-xs text-zinc-500">{status}</div>}
    </div>
  );
}
