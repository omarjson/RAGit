import { useEffect, useRef, useState } from "react";
import { Send } from "lucide-react";
import { chatStream, ragChat, type ChatMessage } from "../lib/ipc";
import { useEngine } from "../state/EngineContext";
import { useLibrary } from "../state/LibraryContext";
import { type IndexLevel } from "../lib/ipc/constants";
import { IndexSidebar } from "./IndexSidebar";

export function Chat() {
  const { status: engine } = useEngine();
  const { refreshFiles } = useLibrary();
  const [msgs, setMsgs] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [ragMode, setRagMode] = useState(false);
  const [rerank, setRerank] = useState(false);
  const [level, setLevel] = useState<IndexLevel>(4);
  const scrollRef = useRef<HTMLDivElement>(null);
  const streamRef = useRef(false);

  const engineUp = engine?.running ?? false;

  useEffect(() => { refreshFiles(); }, [refreshFiles]);
  useEffect(() => { scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight }); }, [msgs]);

  const send = async () => {
    const text = input.trim();
    if (!text || streaming) return;
    const userMsg: ChatMessage = { role: "user", content: text };
    const next = [...msgs, userMsg];
    setMsgs(next);
    setInput("");
    setStreaming(true);
    streamRef.current = true;

    try {
      if (ragMode && engineUp) {
        const answer = await ragChat(text, undefined, msgs, rerank);
        if (streamRef.current) setMsgs((prev) => [...prev, { role: "assistant", content: answer || "(no answer)" }]);
      } else if (engineUp) {
        await chatStream(next, (e) => {
          if (e.event === "token" && streamRef.current) {
            setMsgs((prev) => {
              const copy = [...prev];
              if (copy.length > 0 && copy[copy.length - 1].role === "assistant") {
                copy[copy.length - 1] = { role: "assistant", content: copy[copy.length - 1].content + e.data.text };
              } else {
                copy.push({ role: "assistant", content: e.data.text });
              }
              return copy;
            });
          } else if (e.event === "error" && streamRef.current) {
            setMsgs((prev) => [...prev, { role: "assistant", content: `⚠ ${e.data.message}` }]);
          }
        });
      } else {
        setMsgs((prev) => [...prev, { role: "assistant", content: "⚠ Engine is not running. Launch a model first." }]);
      }
    } catch (err) {
      if (streamRef.current) setMsgs((prev) => [...prev, { role: "assistant", content: `⚠ ${String(err)}` }]);
    } finally {
      setStreaming(false);
      streamRef.current = false;
    }
  };

  return (
    <div className="flex h-full">
      <IndexSidebar
        level={level} onLevelChange={setLevel}
        ragMode={ragMode} onRagModeChange={setRagMode}
        rerank={rerank} onRerankChange={setRerank}
      />
      <div className="flex flex-1 flex-col p-6">
        <h1 className="mb-4 text-2xl font-semibold text-primary">Chat</h1>
        <div ref={scrollRef} className="animate-fade-slide-in flex-1 space-y-4 overflow-auto rounded-sm border border-[#2A2D45] bg-[#141626] p-4">
          {msgs.length === 0 && (
            <div className="text-muted">
              {ragMode ? "Index a folder, then ask away." : "Launch a model from the Model Hub, then ask away."}
            </div>
          )}
          {msgs.filter((m) => m.role !== "system").map((m, i) => (
            <div key={i} className={m.role === "user" ? "text-right" : "text-left"}>
              <div className={
                m.role === "user"
                  ? "inline-block max-w-[80%] rounded-sm bg-brand-fg/20 px-3 py-2 text-sm text-primary"
                  : "inline-block max-w-[80%] rounded-sm bg-[#1C1E32] px-3 py-2 text-sm text-primary"
              }>
                {m.content || "…"}
              </div>
            </div>
          ))}
        </div>
        <div className="mt-3 flex gap-2">
          <input value={input} onChange={(e) => setInput(e.target.value)} onKeyDown={(e) => e.key === "Enter" && send()}
            placeholder="Ask your library…"
            className="input-field flex-1" />
          <button onClick={send} disabled={streaming || !engineUp}
            className="btn-primary">
            <Send size={16} /> Send
          </button>
        </div>
      </div>
    </div>
  );
}
