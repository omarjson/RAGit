import { useEffect, useState } from "react";
import { Image as ImageIcon, AudioLines, Download, CheckCircle2, XCircle, Search } from "lucide-react";
import {
  detectHardware,
  listModels,
  searchModels,
  downloadModel,
  type HardwareInfo,
  type CatalogModel,
  type ModelVariant,
  type SearchHit,
  type DownloadEvent,
} from "../lib/ipc";
import { useEngine } from "../state/EngineContext";

function fmtBytes(n: number): string {
  if (!n || n === 0) return "unknown";
  return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

const fitnessStyle: Record<string, { label: string; cls: string }> = {
  Fits: { label: "fits", cls: "text-emerald-400" },
  Tight: { label: "tight", cls: "text-amber-400" },
  TooBig: { label: "too big", cls: "text-red-400" },
};

function VariantRow({
  model,
  v,
  embedModels,
}: {
  model: CatalogModel;
  v: ModelVariant;
  embedModels: CatalogModel[];
}) {
  const [state, setState] = useState<"idle" | "downloading" | "done" | "error">("idle");
  const [progress, setProgress] = useState(0);
  const { status, start, loading } = useEngine();
  const [embedModel, setEmbedModel] = useState<string>("");

  const fileName = model.defaultFile.replace(/Q4_K_M/i, v.quant);
  const isRunning = status?.running && status?.modelPath?.includes(fileName);

  const handleDownload = () => {
    setState("downloading");
    setProgress(0);
    downloadModel(model.repo, fileName, null, (e: DownloadEvent) => {
      if (e.event === "progress") {
        const pct = e.data.totalBytes > 0 ? (e.data.downloaded / e.data.totalBytes) * 100 : 0;
        setProgress(pct);
      } else if (e.event === "finished") {
        setState("done");
        setProgress(100);
      } else if (e.event === "error") {
        setState("error");
      }
    }).catch(() => setState("error"));
  };

  const handleLaunch = async () => {
    try {
      const emb = embedModel ? `models/${embedModel}` : undefined;
      await start(`models/${fileName}`, undefined, undefined, emb);
    } catch { /* error surfaced in context */ }
  };

  const f = fitnessStyle[v.fitness] ?? { label: "unknown", cls: "text-zinc-500" };
  return (
    <div className="flex items-center justify-between rounded-sm border border-[#2A2D45] bg-[#141626] px-3 py-2 text-sm">
      <div className="flex items-center gap-3">
        <span className="font-mono text-[#D0D2E0]">{v.quant}</span>
        <span className="text-muted">{v.sizeGb.toFixed(1)} GB</span>
        <span className={f.cls}>{f.label}</span>
        {isRunning && (
          <span className="text-emerald-400">
            ● {status!.measuredTps ? `${status!.measuredTps!.toFixed(0)} tok/s` : "starting…"}
          </span>
        )}
      </div>
      <div className="flex items-center gap-2">
        {state === "downloading" && (
          <span className="font-mono text-xs text-muted">{progress.toFixed(0)}%</span>
        )}
        {state === "done" && <CheckCircle2 size={16} className="text-emerald-400" />}
        {state === "error" && <XCircle size={16} className="text-red-400" />}
        <button
          disabled={state === "downloading" || state === "done"}
          onClick={handleDownload}
          className="flex items-center gap-1 rounded-sm bg-brand-fg/80 px-2 py-1 font-mono text-[11px] font-medium text-white hover:bg-brand-fg disabled:opacity-40"
        >
          <Download size={13} /> {state === "done" ? "Saved" : "DL"}
        </button>
        {state === "done" && !isRunning && (
          <button
            disabled={loading}
            onClick={handleLaunch}
            className="btn-ghost text-[11px] font-mono px-2 py-1"
          >
            ▶ Launch
          </button>
        )}
        {embedModels.length > 0 && (
          <select
            value={embedModel}
            onChange={(e) => setEmbedModel(e.target.value)}
            title="Optional dedicated embedding model"
            className="level-option font-mono text-[10px]"
          >
            <option value="">no embed</option>
            {embedModels.map((em) => (
              <option key={em.id} value={em.defaultFile}>
                {em.name}
              </option>
            ))}
          </select>
        )}
      </div>
    </div>
  );
}

export function ModelHub() {
  const [hw, setHw] = useState<HardwareInfo | null>(null);
  const [models, setModels] = useState<CatalogModel[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searching, setSearching] = useState(false);

  useEffect(() => {
    detectHardware().then(setHw).catch(() => {});
    listModels().then(setModels).catch(() => setModels([]));
  }, []);

  const doSearch = async () => {
    const q = query.trim();
    if (!q) { setHits([]); return; }
    setSearching(true);
    try {
      const r = await searchModels(q);
      setHits(r);
    } catch { setHits([]); }
    finally { setSearching(false); }
  };

  const embedModels = models.filter((m) => m.embed);

  return (
    <div className="p-6">
      <h1 className="mb-1 text-2xl font-semibold">Model Hub</h1>
      <p className="mb-6 text-sm text-zinc-400">
        Pick a model, see if your machine can run it, then download and launch it locally.
      </p>

      {hw && (
        <div className="mb-6 grid grid-cols-2 gap-x-8 gap-y-1 rounded-sm border border-[#2A2D45] bg-[#141626] p-4 text-sm text-muted">
          <span>OS / Arch</span><span className="text-[#D0D2E0]">{hw.os} · {hw.arch}</span>
          <span>CPU</span><span className="text-[#D0D2E0]">{hw.cpuBrand} ({hw.cpuCores} cores)</span>
          <span>RAM</span><span className="text-[#D0D2E0]">{fmtBytes(hw.totalRamBytes)}</span>
          <span>GPU</span>
          <span className="text-[#D0D2E0]">{hw.gpuName ?? "unknown"} · {fmtBytes(hw.vramBytes ?? 0)} VRAM</span>
        </div>
      )}

      <div className="mb-6 flex gap-2">
        <div className="input-field flex flex-1 items-center gap-2">
          <Search size={16} className="text-muted" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && doSearch()}
            placeholder="Search HuggingFace for any GGUF model (e.g. Llama-3.1-8B, Mistral-Nemo)…"
            className="flex-1 bg-transparent text-sm text-primary outline-none placeholder:text-[#5A5C72]"
          />
        </div>
        <button
          onClick={doSearch}
          disabled={searching}
          className="btn-primary px-4 py-2"
        >
          {searching ? "Searching…" : "Search"}
        </button>
      </div>

      {hits.length > 0 && (
        <div className="mb-6 flex flex-col gap-2">
          <div className="font-mono text-[11px] text-muted">Search results ({hits.length})</div>
          {hits.map((h) => <SearchHitRow key={h.id} hit={h} />)}
        </div>
      )}

      <div className="flex flex-col gap-4">
        {models.map((m) => (
          <div key={m.id} className="rounded-sm border border-[#2A2D45] bg-[#141626] p-4">
            <div className="mb-3 flex items-center gap-2">
              <span className="font-semibold text-[#E2E4F0]">{m.name}</span>
              {m.modalities.includes("vision") && <ImageIcon size={15} className="text-sky-400" />}
              {m.modalities.includes("audio") && <AudioLines size={15} className="text-violet-400" />}
              {m.embed && <span className="rounded-sm bg-[#1C1E32] px-1.5 py-0.5 font-mono text-[10px] text-muted">embed</span>}
              <span className="ml-auto font-mono text-[11px] text-muted">ctx {m.context.toLocaleString()}</span>
            </div>
            <div className="flex flex-col gap-1.5">
              {m.variants.map((v) => (
                <VariantRow key={v.quant} model={m} v={v} embedModels={embedModels} />
              ))}
            </div>
          </div>
        ))}
        {models.length === 0 && (
          <div className="rounded-sm border border-dashed border-[#2A2D45] p-10 text-center text-muted">
            Loading catalog…
          </div>
        )}
      </div>
    </div>
  );
}

function SearchHitRow({ hit }: { hit: SearchHit }) {
  const [state, setState] = useState<"idle" | "downloading" | "done" | "error">("idle");
  const [progress, setProgress] = useState(0);
  const f = fitnessStyle[hit.fitness] ?? { label: "unknown", cls: "text-zinc-500" };

  const handle = () => {
    setState("downloading");
    setProgress(0);
    downloadModel(hit.repo, hit.defaultFile, null, (e: DownloadEvent) => {
      if (e.event === "progress") {
        const pct = e.data.totalBytes > 0 ? (e.data.downloaded / e.data.totalBytes) * 100 : 0;
        setProgress(pct);
      } else if (e.event === "finished") {
        setState("done"); setProgress(100);
      } else if (e.event === "error") {
        setState("error");
      }
    }).catch(() => setState("error"));
  };

  return (
    <div className="flex items-center justify-between rounded-sm border border-[#2A2D45] bg-[#141626] px-3 py-2 text-sm">
      <div className="flex min-w-0 items-center gap-3">
        <span className="truncate font-mono text-[#D0D2E0]">{hit.repo}</span>
        <span className="text-muted">{hit.sizeGb.toFixed(1)} GB</span>
        <span className={f.cls}>{f.label}</span>
        <span className="font-mono text-[11px] text-muted">{hit.downloads.toLocaleString()} downloads</span>
      </div>
      <div className="flex items-center gap-2">
        {state === "downloading" && <span className="text-xs text-zinc-400">{progress.toFixed(0)}%</span>}
        {state === "done" && <CheckCircle2 size={16} className="text-emerald-400" />}
        {state === "error" && <XCircle size={16} className="text-red-400" />}
        <button
          disabled={state === "downloading" || state === "done"}
          onClick={handle}
          className="flex items-center gap-1 rounded bg-brand-fg/80 px-2 py-1 text-xs font-medium text-white hover:bg-brand-fg disabled:opacity-40"
        >
          <Download size={14} /> {state === "done" ? "Saved" : "Download"}
        </button>
      </div>
    </div>
  );
}
