import { useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { ModelHub } from "./components/ModelHub";
import { Library } from "./components/Library";
import { Indexing } from "./components/Indexing";
import { Chat } from "./components/Chat";
import { Settings } from "./components/Settings";
import { Team } from "./components/Team";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { runningInTauri } from "./lib/ipc";
import { EngineProvider } from "./state/EngineContext";
import { LibraryProvider } from "./state/LibraryContext";
import { SettingsProvider } from "./state/SettingsContext";
import { TeamProvider } from "./state/TeamContext";

export type Screen = "models" | "library" | "indexing" | "chat" | "settings" | "team";

export default function App() {
  const [screen, setScreen] = useState<Screen>("models");

  if (!runningInTauri()) {
    return (
      <div className="flex h-screen w-screen flex-col items-center justify-center gap-4 bg-deep p-8 text-center text-primary">
        <h1 className="text-2xl font-semibold text-primary">RAGit</h1>
        <p className="max-w-md text-sm text-muted">
          This app must run inside the Tauri desktop runtime. Opening the built
          page in a browser will not work — the native backend (LLM engine, indexer,
          and file access) is unavailable.
        </p>
        <p className="max-w-md text-sm text-accent">
          Run it from the project root with:
        </p>
        <pre className="rounded-sm border border-[#2A2D45] bg-[#141626] px-4 py-2 font-mono text-xs text-emerald-400">
          npm run tauri dev
        </pre>
        <p className="text-xs text-muted">or launch the installed RAGit app.</p>
      </div>
    );
  }

  return (
    <ErrorBoundary>
      <EngineProvider>
        <LibraryProvider>
          <SettingsProvider>
            <TeamProvider>
              <div className="flex h-screen w-screen bg-deep text-primary">
                <Sidebar screen={screen} onNavigate={setScreen} />
                <main className="scan-line flex-1 overflow-auto">
                  {screen === "models" && <ModelHub />}
                  {screen === "library" && <Library />}
                  {screen === "indexing" && <Indexing />}
                  {screen === "chat" && <Chat />}
                  {screen === "settings" && <Settings />}
                  {screen === "team" && <Team />}
                </main>
              </div>
            </TeamProvider>
          </SettingsProvider>
        </LibraryProvider>
      </EngineProvider>
    </ErrorBoundary>
  );
}
