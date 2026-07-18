import { useEffect, useRef, useState } from "react";
import { Send, FolderPlus, Database, Pause, Play, Square } from "lucide-react";
import {
  chatStream,
  ChatMessage,
  ChatEvent,
  engineStatus,
  indexLibrary,
  ragChat,
  openFolderDialog,
  pauseIndex,
  resumeIndex,
  cancelIndex,
  listIndexedFiles,
  setScheduler,
  exportLibrary,
  importLibrary,
  IndexProgress,
  IndexedFile,
} from "../lib/ipc";

interface Turn {
  role: "user" | "assistant";
  content: string;
}

const LEVEL_LABELS = ["", "L1 raw", "L2 structure", "L3 summaries", "L4 dense", "L5 rerank"];

export function Chat() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [turns, setTurns] = useState<Turn[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [engineUp, setEngineUp] = useState(false);
  const [ragMode, setRagMode] = useState(false);
  const [rerank, setRerank] = useState(false);
  const [libId] = useState<string>("default");
  const [status, setStatus] = useState<string>("");
  const [level, setLevel] = useState(4);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [jobActive, setJobActive] = useState(false);
  const [files, setFiles] = useState<IndexedFile[]>([]);
  const [schedulerOn, setSchedulerOn] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    engineStatus().then((s) => setEngineUp(s.running)).catch(() => setEngineUp(false));
    listIndexedFiles(libId).then(setFiles).catch(() => {});
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [turns]);

  const refreshFiles = () => listIndexedFiles(libId).then(setFiles).catch(() => {});

  const indexFolder = async () => {
    const dir = await openFolderDialog();
    if (!dir) return;
    setJobActive(true);
    setProgress(null);
    try {
      const msg = await indexLibrary(dir, libId, level, (p) => {
        setProgress(p);
        if (p.status === "done" || p.status === "canceled") {
          setJobActive(false);
          refreshFiles();
        }
      });
      setStatus(msg);
      setRagMode(true);
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
      setJobActive(false);
    }
  };

  const toggleScheduler = async () => {
    try {
      const msg = await setScheduler(libId, !schedulerOn, 60);
      setSchedulerOn((v) => !v);
      setStatus(msg);
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
    }
  };

  const doExport = async () => {
    const p = await import("@tauri-apps/plugin-dialog").then((m) =>
      m.save({ defaultPath: `${libId}.json`, filters: [{ name: "JSON", extensions: ["json"] }] })
    );
    if (!p) return;
    try {
      setStatus(await exportLibrary(p as string, libId));
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
    }
  };

  const doImport = async () => {
    const p = await openFolderDialog();
    // openFolderDialog returns a dir; for a file we need open (file). Use raw import.
    void p;
    const f = await import("@tauri-apps/plugin-dialog").then((m) =>
      m.open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] })
    );
    if (!f) return;
    try {
      setStatus(await importLibrary(f as string, libId));
      refreshFiles();
    } catch (e) {
      setStatus(`⚠ ${String(e)}`);
    }
  };

  const send = async () => {
    const text = input.trim();
    if (!text || streaming) return;
    const next: ChatMessage[] = [...messages, { role: "user", content: text }];
    setMessages(next);
    setTurns([...turns, { role: "user", content: text }, { role: "assistant", content: "" }]);
    setInput("");
    setStreaming(true);

    const assistantIdx = turns.length + 1;
    const setAssistant = (content: string) =>
      setTurns((prev) => {
        const copy = [...prev];
        copy[assistantIdx] = { role: "assistant", content };
        return copy;
      });

    try {
      if (ragMode && engineUp) {
        const answer = await ragChat(text, libId, messages, rerank);
        setAssistant(answer || "(no answer)");
      } else if (engineUp) {
        await chatStream(next, (e: ChatEvent) => {
          if (e.event === "token") {
            setTurns((prev) => {
              const copy = [...prev];
              copy[assistantIdx] = {
                role: "assistant",
                content: copy[assistantIdx].content + e.data.text,
              };
              return copy;
            });
          } else if (e.event === "error") {
            setAssistant(`⚠ ${e.data.message}`);
          }
        });
      } else {
        setAssistant("⚠ Engine is not running. Launch a model first.");
      }
    } catch (err) {
      setAssistant(`⚠ ${String(err)}`);
    } finally {
      setStreaming(false);
    }
  };

  const pct = progress && progress.total > 0
    ? Math.round((progress.processed / progress.total) * 100)
    : 0;

  return (
    <div className="flex h-full">
      <div className="flex w-72 flex-col gap-3 border-r border-zinc-800 p-4">
        <button
          onClick={indexFolder}
          disabled={jobActive}
          className="flex items-center justify-center gap-2 rounded-md border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-800 disabled:opacity-40"
        >
          <FolderPlus size={16} /> Index Folder
        </button>

        <div>
          <label className="text-xs text-zinc-400">Depth level</label>
          <select
            value={level}
            onChange={(e) => setLevel(Number(e.target.value))}
            className="mt-1 w-full rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-sm text-zinc-100"
          >
            {[1, 2, 3, 4, 5].map((l) => (
              <option key={l} value={l}>
                {LEVEL_LABELS[l]}
              </option>
            ))}
          </select>
        </div>

        {jobActive && (
          <div className="space-y-2 rounded-md border border-zinc-700 p-2">
            <div className="text-xs text-zinc-300">
              {progress?.status} · {progress?.processed}/{progress?.total} · {LEVEL_LABELS[progress?.level ?? 0]}
            </div>
            <div className="h-2 w-full overflow-hidden rounded bg-zinc-800">
              <div className="h-full bg-brand-fg" style={{ width: `${pct}%` }} />
            </div>
            <div className="text-[11px] text-zinc-500 truncate">{progress?.message}</div>
            <div className="flex gap-2">
              <button onClick={() => pauseIndex(libId)} className="flex-1 rounded bg-zinc-800 px-2 py-1 text-xs hover:bg-zinc-700">
                <Pause size={12} className="inline" /> Pause
              </button>
              <button onClick={() => resumeIndex(libId)} className="flex-1 rounded bg-zinc-800 px-2 py-1 text-xs hover:bg-zinc-700">
                <Play size={12} className="inline" /> Resume
              </button>
              <button onClick={() => cancelIndex(libId)} className="flex-1 rounded bg-zinc-800 px-2 py-1 text-xs hover:bg-zinc-700">
                <Square size={12} className="inline" /> Cancel
              </button>
            </div>
          </div>
        )}

        <label className="flex items-center gap-2 text-sm text-zinc-300">
          <input type="checkbox" checked={ragMode} onChange={(e) => setRagMode(e.target.checked)} />
          RAG mode (retrieve context)
        </label>
        <label className="flex items-center gap-2 text-sm text-zinc-300">
          <input type="checkbox" checked={schedulerOn} onChange={toggleScheduler} />
          Auto-reindex (scheduler)
        </label>
        <label className="flex items-center gap-2 text-sm text-zinc-300">
          <input type="checkbox" checked={rerank} onChange={(e) => setRerank(e.target.checked)} />
          Rerank retrieved chunks
        </label>

        <div className="flex gap-2">
          <button onClick={doExport} className="flex-1 rounded-md border border-zinc-700 px-2 py-1 text-xs text-zinc-300 hover:bg-zinc-800">
            Export
          </button>
          <button onClick={doImport} className="flex-1 rounded-md border border-zinc-700 px-2 py-1 text-xs text-zinc-300 hover:bg-zinc-800">
            Import
          </button>
        </div>

        <div className="flex items-center gap-2 text-sm text-zinc-300">
          <Database size={16} />
          <span>Library: {libId}</span>
        </div>

        <div className="flex-1 overflow-auto rounded-md border border-zinc-800 p-2">
          <div className="mb-1 text-xs text-zinc-400">Indexed files ({files.length})</div>
          {files.map((f) => (
            <div key={f.id} className="flex items-center justify-between py-0.5 text-[11px]">
              <span className="truncate text-zinc-300">{f.fileName}</span>
              <span className={f.status === "done" ? "text-emerald-400" : f.status === "error" ? "text-red-400" : "text-zinc-500"}>
                {f.status} · L{f.level} · {f.chunks}
              </span>
            </div>
          ))}
        </div>

        {status && <div className="text-xs text-zinc-500">{status}</div>}
      </div>

      <div className="flex flex-1 flex-col p-6">
        <div className="mb-2 flex items-center gap-2">
          <h1 className="text-2xl font-semibold">Chat</h1>
          <span className={engineUp ? "text-emerald-400" : "text-zinc-600"}>
            {engineUp ? "● engine live" : "○ engine off"}
          </span>
          {ragMode && <span className="text-brand-fg">· RAG</span>}
        </div>

        <div ref={scrollRef} className="flex-1 space-y-4 overflow-auto rounded-lg border border-zinc-800 bg-zinc-900/30 p-4">
          {turns.length === 0 && (
            <div className="text-zinc-500">
              Launch a model from the Model Hub{ragMode ? " and index a folder" : ""}, then ask away.
            </div>
          )}
          {turns.map((t, i) => (
            <div key={i} className={t.role === "user" ? "text-right" : "text-left"}>
              <div
                className={
                  t.role === "user"
                    ? "inline-block max-w-[80%] rounded-lg bg-brand-fg/20 px-3 py-2 text-sm text-zinc-100"
                    : "inline-block max-w-[80%] rounded-lg bg-zinc-800 px-3 py-2 text-sm text-zinc-100"
                }
              >
                {t.content || (t.role === "assistant" ? "…" : "")}
              </div>
            </div>
          ))}
        </div>

        <div className="mt-3 flex gap-2">
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && send()}
            placeholder="Ask your library…"
            className="flex-1 rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-brand-fg"
          />
          <button
            onClick={send}
            disabled={streaming || !engineUp}
            className="flex items-center gap-1 rounded-md bg-brand-fg px-3 py-2 text-sm font-medium text-white hover:bg-brand-fg/80 disabled:opacity-40"
          >
            <Send size={16} /> Send
          </button>
        </div>
      </div>
    </div>
  );
}
