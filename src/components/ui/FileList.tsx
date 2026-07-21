import { FileWarning } from "lucide-react";
import type { IndexedFile } from "../../lib/ipc";

export function FileList({ files }: { files: IndexedFile[] }) {
  if (files.length === 0) {
    return <div className="py-6 text-center text-xs text-muted">No files indexed yet.</div>;
  }
  return (
    <div className="space-y-0.5">
      {files.map((f) => (
        <div key={f.id} className="flex items-center justify-between rounded-sm border border-[#2A2D45] bg-[#141626] px-3 py-1.5 text-sm">
          <span className="truncate text-[#D0D2E0]">{f.fileName}</span>
          <div className="flex items-center gap-2 text-[11px] font-mono">
            <span className={
              f.status === "done" ? "text-emerald-400" : f.status === "error" ? "text-red-400" : "text-muted"
            }>
              {f.status} · L{f.level} · {f.chunks}
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
  );
}

export function FileListCompact({ files }: { files: IndexedFile[] }) {
  return (
    <div className="overflow-auto rounded-sm border border-[#2A2D45] p-2">
      <div className="mb-1 text-xs text-muted">Indexed files ({files.length})</div>
      {files.map((f) => (
        <div key={f.id} className="flex items-center justify-between py-0.5 font-mono text-[11px]">
          <span className="truncate text-[#A5ABB8]">{f.fileName}</span>
            <span className={f.status === "done" ? "text-emerald-400" : f.status === "error" ? "text-red-400" : "text-muted"}>
            {f.status} · L{f.level} · {f.chunks}
          </span>
        </div>
      ))}
    </div>
  );
}
