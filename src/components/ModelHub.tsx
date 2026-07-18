import { useEffect, useState } from "react";
import { Image as ImageIcon, AudioLines, Download, CheckCircle2, XCircle } from "lucide-react";
import {
  detectHardware,
  listModels,
  downloadModel,
  startEngine,
  HardwareInfo,
  CatalogModel,
  ModelVariant,
  Fitness,
  DownloadEvent,
  EngineStatus,
} from "../lib/ipc";

function fmtBytes(n: number): string {
  if (!n || n === 0) return "unknown";
  return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

const fitnessStyle: Record<Fitness, { label: string; cls: string }> = {
  Fits: { label: "🟢 fits", cls: "text-emerald-400" },
  Tight: { label: "🟡 tight", cls: "text-amber-400" },
  TooBig: { label: "🔴 too big", cls: "text-red-400" },
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
  const [launching, setLaunching] = useState(false);
  const [engine, setEngine] = useState<EngineStatus | null>(null);
  const [embedModel, setEmbedModel] = useState<string>("");

  const fileName = model.defaultFile.replace(/Q4_K_M/i, v.quant);

  const handle = () => {
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
    }).catch(() => {
      setState("error");
    });
  };

  const launch = async () => {
    setLaunching(true);
    try {
      const emb = embedModel ? `models/${embedModel}` : undefined;
      const st = await startEngine(`models/${fileName}`, undefined, undefined, emb);
      setEngine(st);
    } catch (e) {
      setEngine({ running: false, modelPath: null, port: null, measuredTps: null, backend: String(e), embedModel: null, embedPort: null });
    } finally {
      setLaunching(false);
    }
  };

  const f = fitnessStyle[v.fitness];
  return (
    <div className="flex items-center justify-between rounded-md border border-zinc-800 bg-zinc-900/40 px-3 py-2 text-sm">
      <div className="flex items-center gap-3">
        <span className="font-mono text-zinc-200">{v.quant}</span>
        <span className="text-zinc-500">{v.sizeGb.toFixed(1)} GB</span>
        <span className={f.cls}>{f.label}</span>
        {engine?.running && (
          <span className="text-emerald-400">
            ● {engine.measuredTps ? `${engine.measuredTps.toFixed(0)} tok/s` : "starting…"}
          </span>
        )}
      </div>
      <div className="flex items-center gap-2">
        {state === "downloading" && (
          <span className="text-xs text-zinc-400">{progress.toFixed(0)}%</span>
        )}
        {state === "done" && <CheckCircle2 size={16} className="text-emerald-400" />}
        {state === "error" && <XCircle size={16} className="text-red-400" />}
        <button
          disabled={state === "downloading" || state === "done"}
          onClick={handle}
          className="flex items-center gap-1 rounded bg-brand-fg/80 px-2 py-1 text-xs font-medium text-white hover:bg-brand-fg disabled:opacity-40"
        >
          <Download size={14} /> {state === "done" ? "Saved" : "Download"}
        </button>
        {state === "done" && !engine?.running && (
          <button
            disabled={launching}
            onClick={launch}
            className="flex items-center gap-1 rounded border border-zinc-700 px-2 py-1 text-xs font-medium text-zinc-200 hover:bg-zinc-800 disabled:opacity-40"
          >
            ▶ Launch
          </button>
        )}
        {embedModels.length > 0 && (
          <select
            value={embedModel}
            onChange={(e) => setEmbedModel(e.target.value)}
            title="Optional dedicated embedding model"
            className="rounded border border-zinc-700 bg-zinc-900 px-1 py-1 text-[10px] text-zinc-300"
          >
            <option value="">no embed model</option>
            {embedModels.map((em) => (
              <option key={em.id} value={em.defaultFile}>
                embed: {em.name}
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

  useEffect(() => {
    detectHardware().then(setHw).catch(() => {});
    listModels().then(setModels).catch(() => {});
  }, []);

  const embedModels = models.filter((m) => m.embed);

  return (
    <div className="p-6">
      <h1 className="mb-1 text-2xl font-semibold">Model Hub</h1>
      <p className="mb-6 text-sm text-zinc-400">
        Pick a model, see if your machine can run it, then download and launch it locally.
      </p>

      {hw && (
        <div className="mb-6 grid grid-cols-2 gap-x-8 gap-y-1 rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 text-sm text-zinc-400">
          <span>OS / Arch</span><span className="text-zinc-200">{hw.os} · {hw.arch}</span>
          <span>CPU</span><span className="text-zinc-200">{hw.cpuBrand} ({hw.cpuCores} cores)</span>
          <span>RAM</span><span className="text-zinc-200">{fmtBytes(hw.totalRamBytes)}</span>
          <span>GPU</span>
          <span className="text-zinc-200">{hw.gpuName ?? "unknown"} · {fmtBytes(hw.vramBytes ?? 0)} VRAM</span>
        </div>
      )}

      <div className="flex flex-col gap-4">
        {models.map((m) => (
          <div key={m.id} className="rounded-lg border border-zinc-800 bg-zinc-900/30 p-4">
            <div className="mb-3 flex items-center gap-2">
              <span className="font-semibold text-zinc-100">{m.name}</span>
              {m.modalities.includes("vision") && <ImageIcon size={15} className="text-sky-400" />}
              {m.modalities.includes("audio") && <AudioLines size={15} className="text-violet-400" />}
              {m.embed && <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400">embed</span>}
              <span className="ml-auto text-xs text-zinc-500">ctx {m.context.toLocaleString()}</span>
            </div>
            <div className="flex flex-col gap-1.5">
              {m.variants.map((v) => (
                <VariantRow key={v.quant} model={m} v={v} embedModels={embedModels} />
              ))}
            </div>
          </div>
        ))}
        {models.length === 0 && (
          <div className="rounded-lg border border-dashed border-zinc-800 p-10 text-center text-zinc-500">
            Loading catalog…
          </div>
        )}
      </div>
    </div>
  );
}
