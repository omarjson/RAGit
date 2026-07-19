import { LEVELS, LEVEL_LABELS, type IndexLevel } from "../../lib/ipc/constants";

export function LevelSelect({ value, onChange }: { value: IndexLevel; onChange: (v: IndexLevel) => void }) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(Number(e.target.value) as IndexLevel)}
      className="rounded-sm border border-[#2A2D45] bg-[#1C1E32] px-2 py-1 font-mono text-xs text-[#E2E4F0] outline-none transition-colors duration-150 focus:border-brand-fg/50"
    >
      {LEVELS.map((l) => (
        <option key={l} value={l}>{LEVEL_LABELS[l]}</option>
      ))}
    </select>
  );
}
