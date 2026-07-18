import { useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { ModelHub } from "./components/ModelHub";
import { Library } from "./components/Library";
import { Indexing } from "./components/Indexing";
import { Chat } from "./components/Chat";
import { Settings } from "./components/Settings";
import { Team } from "./components/Team";

export type Screen = "models" | "library" | "indexing" | "chat" | "settings" | "team";

export default function App() {
  const [screen, setScreen] = useState<Screen>("models");

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
