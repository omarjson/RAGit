import { useEffect, useState } from "react";
import { FolderPlus, Upload, Download } from "lucide-react";
import { exportLibrary, importLibrary, openFolderDialog } from "../lib/ipc";
import { useLibrary } from "../state/LibraryContext";
import { LEVEL_LABELS, type IndexLevel } from "../lib/ipc/constants";
import { ProgressBar } from "./ui/ProgressBar";
import { LevelSelect } from "./ui/LevelSelect";
import { FileList } from "./ui/FileList";

export function Library() {
  const { files, startIndex, job, refreshFiles } = useLibrary();
  const [level, setLevel] = useState<IndexLevel>(4);
  const [status, setStatus] = useState("");
  const [dragging, setDragging] = useState(false);

  useEffect(() => { refreshFiles(); }, [refreshFiles]);

  const runIndex = async (path: string) => {
    try {
      await startIndex(path, undefined, level);
      setStatus("");
    } catch (e) { setStatus(`⚠ ${String(e)}`); }
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
      const path: string | undefined = item.path ?? item.webkitRelativePath ?? item.name;
      if (path) await runIndex(path);
    }
  };

  const doExport = async () => {
    const p = await import("@tauri-apps/plugin-dialog").then((m) =>
      m.save({ defaultPath: "default.json", filters: [{ name: "JSON", extensions: ["json"] }] })
    );
    if (!p) return;
    try { setStatus(await exportLibrary(p as string)); } catch (e) { setStatus(`⚠ ${String(e)}`); }
  };

  const doImport = async () => {
    const f = await import("@tauri-apps/plugin-dialog").then((m) =>
      m.open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] })
    );
    if (!f) return;
    try {
      setStatus(await importLibrary(f as string));
      refreshFiles();
    } catch (e) { setStatus(`⚠ ${String(e)}`); }
  };

  const pct = job && job.total > 0 ? Math.round((job.processed / job.total) * 100) : 0;

  return (
    <div className="p-6">
      <h1 className="mb-1 text-2xl font-semibold text-primary">Library</h1>
      <p className="mb-6 text-sm text-muted">Drag & drop files or a folder to build your knowledge base.</p>

      <div className="mb-4 flex gap-3">
        <button onClick={indexFolder} className="btn-ghost text-sm">
          <FolderPlus size={16} /> Select Folder
        </button>
        <div className="flex items-center gap-2">
          <label className="text-xs text-muted">Depth level</label>
          <LevelSelect value={level} onChange={setLevel} />
        </div>
        <div className="ml-auto flex gap-2">
          <button onClick={doExport} className="btn-ghost text-xs px-3 py-1">
            <Download size={13} /> Export
          </button>
          <button onClick={doImport} className="btn-ghost text-xs px-3 py-1">
            <Upload size={13} /> Import
          </button>
        </div>
      </div>

      <div
        onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
        onDragLeave={() => setDragging(false)}
        onDrop={onDrop}
        className={`mb-6 rounded-sm border border-dashed p-10 text-center transition-colors ${
          dragging ? "border-brand-fg bg-brand-fg/5 text-primary" : "border-[#2A2D45] text-muted"
        }`}>
        Drop files or folders here (currently {LEVEL_LABELS[level]})
      </div>

      {job && (
        <div className="mb-6 rounded-sm border border-[#2A2D45] bg-[#141626] p-3">
          <div className="mb-1 flex items-center justify-between font-mono text-[11px] text-[#A5ABB8]">
            <span className="truncate">{job.currentFile}</span>
            <span className="text-muted">{job.status} · {job.processed}/{job.total}</span>
          </div>
          <ProgressBar percent={pct} />
          {job.message && <div className="mt-1 truncate font-mono text-[11px] text-muted">{job.message}</div>}
        </div>
      )}

      <div className="rounded-sm border border-[#2A2D45] bg-[#141626] p-4">
        <div className="mb-2 text-sm text-muted">Indexed files ({files.length})</div>
        <FileList files={files} />
      </div>

      {status && <div className="mt-4 font-mono text-[11px] text-muted">{status}</div>}
    </div>
  );
}
