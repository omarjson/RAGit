import { useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { ModelHub } from "./components/ModelHub";
import { Library } from "./components/Library";
import { Indexing } from "./components/Indexing";
import { Chat } from "./components/Chat";
import { Settings } from "./components/Settings";
import { Team } from "./components/Team";
import { runningInTauri } from "./lib/ipc";

export type Screen = "models" | "library" | "indexing" | "chat" | "settings" | "team";

export default function App() {
  const [screen, setScreen] = useState<Screen>("models");

  if (!runningInTauri()) {
    return (
      <div className="flex h-screen w-screen flex-col items-center justify-center gap-4 bg-zinc-950 p-8 text-center text-zinc-200">
        <h1 className="text-2xl font-semibold text-zinc-100">RAGit</h1>
        <p className="max-w-md text-sm text-zinc-400">
          This app must run inside the Tauri desktop runtime. Opening the built
          page in a browser will not work — the native backend (LLM engine, indexer,
          and file access) is unavailable.
        </p>
        <p className="max-w-md text-sm text-amber-400">
          Run it from the project root with:
        </p>
        <pre className="rounded-md border border-zinc-800 bg-zinc-900 px-4 py-2 text-xs text-emerald-400">
          npm run tauri dev
        </pre>
        <p className="text-xs text-zinc-600">or launch the installed RAGit app.</p>
      </div>
    );
  }

  return (
    <div className="flex h-screen w-screen bg-zinc-950 text-zinc-100">
      <Sidebar screen={screen} onNavigate={setScreen} />
      <main className="flex-1 overflow-auto">
        {screen === "models" && <ModelHub />}
        {screen === "library" && <Library />}
        {screen === "indexing" && <Indexing />}
        {screen === "chat" && <Chat />}
        {screen === "settings" && <Settings />}
        {screen === "team" && <Team />}
      </main>
    </div>
  );
}
