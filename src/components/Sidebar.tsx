import { Cpu, Library as LibraryIcon, ListTree, MessageSquare, Settings as SettingsIcon, Users } from "lucide-react";
import type { Screen } from "../App";

const items: { id: Screen; label: string; icon: typeof Cpu }[] = [
  { id: "models", label: "Models", icon: Cpu },
  { id: "library", label: "Library", icon: LibraryIcon },
  { id: "indexing", label: "Indexing", icon: ListTree },
  { id: "chat", label: "Chat", icon: MessageSquare },
  { id: "team", label: "Team", icon: Users },
  { id: "settings", label: "Settings", icon: SettingsIcon },
];

export function Sidebar({ screen, onNavigate }: { screen: Screen; onNavigate: (s: Screen) => void }) {
  return (
    <aside className="flex w-56 flex-col border-r border-zinc-800 bg-zinc-900/60 p-3">
      <div className="mb-6 flex items-center gap-2 px-2 text-lg font-semibold">
        <span className="text-brand-fg">🐙</span> RAGit
      </div>
      <nav className="flex flex-col gap-1">
        {items.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => onNavigate(id)}
            className={`flex items-center gap-3 rounded-md px-3 py-2 text-left text-sm transition ${
              screen === id ? "bg-zinc-800 text-white" : "text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200"
            }`}
          >
            <Icon size={18} /> {label}
          </button>
        ))}
      </nav>
      <div className="mt-auto rounded-md bg-zinc-900 p-3 text-xs text-zinc-500">
        Active model
        <div className="mt-1 font-medium text-zinc-300">none</div>
        <div className="mt-1 text-zinc-500">● stopped</div>
      </div>
    </aside>
  );
}
