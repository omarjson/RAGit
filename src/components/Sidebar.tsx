import { Cpu, Library as LibraryIcon, ListTree, MessageSquare, Settings as SettingsIcon, Users } from "lucide-react";
import type { Screen } from "../App";
import { useEngine } from "../state/EngineContext";

const items: { id: Screen; label: string; icon: typeof Cpu }[] = [
  { id: "models", label: "Models", icon: Cpu },
  { id: "library", label: "Library", icon: LibraryIcon },
  { id: "indexing", label: "Indexing", icon: ListTree },
  { id: "chat", label: "Chat", icon: MessageSquare },
  { id: "team", label: "Team", icon: Users },
  { id: "settings", label: "Settings", icon: SettingsIcon },
];

export function Sidebar({ screen, onNavigate }: { screen: Screen; onNavigate: (s: Screen) => void }) {
  const { status } = useEngine();
  const model = status?.modelPath ? status.modelPath.split(/[/\\]/).pop() : "none";
  const running = status?.running ?? false;

  return (
    <aside className="flex w-56 flex-col border-r border-[#2A2D45] bg-[#0B0D17] p-3">
      <div className="mb-8 mt-1 flex items-center gap-2 px-2">
        <span className="inline-flex h-6 w-6 items-center justify-center rounded-sm border border-brand-fg/30 bg-brand-fg/10 font-mono text-[11px] font-medium leading-none text-brand-fg">R</span>
        <span className="font-sans text-sm font-semibold tracking-wide text-[#E2E4F0]">RAGit</span>
      </div>
      <nav className="flex flex-col gap-0.5">
        {items.map(({ id, label, icon: Icon }) => {
          const active = screen === id;
          return (
            <button
              key={id}
              onClick={() => onNavigate(id)}
              className={`relative flex items-center gap-3 rounded-sm px-3 py-2 text-left text-sm transition-all duration-150 ${
                active
                  ? "bg-[#1C1E32] text-[#E2E4F0]"
                  : "text-muted hover:bg-[#1C1E32]/50 hover:text-[#E2E4F0]"
              }`}
            >
              {active && <span className="nav-indicator active" />}
              <Icon size={16} className={active ? "text-brand-fg" : ""} />
              <span className={active ? "font-medium" : ""}>{label}</span>
            </button>
          );
        })}
      </nav>
      <div className="mt-auto rounded-sm border border-[#2A2D45] bg-[#141626] p-3">
        <div className="truncate font-mono text-[11px] font-medium text-[#8B8DA6]" title={status?.modelPath ?? undefined}>
          {model}
        </div>
        <div className={`mt-1.5 flex items-center gap-2 ${running ? "text-brand-fg" : "text-muted"}`}>
          <span className={`inline-block h-1.5 w-1.5 rounded-full ${running ? "bg-brand-fg animate-pulse-slow" : "bg-[#5A5C72]"}`} />
          <span className="font-mono text-[10px] uppercase tracking-wider">
            {running ? `port ${status!.port}` : "stopped"}
          </span>
        </div>
      </div>
    </aside>
  );
}
