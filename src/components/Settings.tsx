import { useEffect, useState } from "react";
import { Cpu, Server, Play, Square, LogIn, UserPlus, X } from "lucide-react";
import { detectHardware, listModels, type HardwareInfo, type CatalogModel } from "../lib/ipc";
import { useEngine } from "../state/EngineContext";
import { useSettings } from "../state/SettingsContext";
import { useTeam } from "../state/TeamContext";

function fmtBytes(n: number | null | undefined): string {
  if (!n || n === 0) return "unknown";
  return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-muted">{label}</span>
      <span className="text-right text-[#D0D2E0]">{value}</span>
    </div>
  );
}

function Card({ title, icon, children }: { title: string; icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="rounded-sm border border-[#2A2D45] bg-[#141626] p-4">
      <div className="mb-3 flex items-center gap-2 text-[#E2E4F0]">
        {icon}<h2 className="text-sm font-semibold">{title}</h2>
      </div>
      <div className="flex flex-col gap-2 text-sm">{children}</div>
    </div>
  );
}

export function Settings() {
  const { settings, update } = useSettings();
  const { status: engine, loading: engBusy, start, stop, error: engErr } = useEngine();
  const { running: teamOn, token, role, users, me, loading: teamBusy, error: teamErr, start: teamStart, stop: teamStop, login, register, logout, setUserRole } = useTeam();
  const [hw, setHw] = useState<HardwareInfo | null>(null);
  const [models, setModels] = useState<CatalogModel[]>([]);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [hwErr, setHwErr] = useState<string | null>(null);
  const [catalogErr, setCatalogErr] = useState<string | null>(null);

  useEffect(() => {
    detectHardware().then(setHw).catch((e) => setHwErr(String(e)));
    listModels().then((m) => {
      setModels(m);
      if (!settings.selectedModel && m.length > 0) update({ selectedModel: m[0].defaultFile });
    }).catch((e) => setCatalogErr(String(e)));
  }, []);

  return (
    <div className="p-6">
      <h1 className="mb-1 text-2xl font-semibold">Settings</h1>
      <p className="mb-6 text-sm text-zinc-400">Mode (Local / Team), model, GPU layers, context size, hardware, and team management.</p>

      <div className="mb-6 flex items-center gap-3 rounded-sm border border-[#2A2D45] bg-[#141626] p-3 text-sm">
        <span className="text-muted">Mode</span>
        <div className="flex gap-2">
          {(["local", "team"] as const).map((m) => (
            <button key={m} onClick={() => update({ mode: m })}
              className={`rounded-sm px-3 py-1 font-mono text-xs font-medium capitalize ${
                settings.mode === m ? "bg-brand-fg text-white" : "border border-[#3A3D5A] text-muted hover:bg-[#1C1E32] hover:text-[#E2E4F0]"
              }`}>{m}</button>
          ))}
        </div>
      </div>

      {engErr && <div className="mb-4 rounded-sm border border-red-800 bg-red-900/20 p-3 text-sm text-red-300">{engErr}</div>}
      {hwErr && <div className="mb-4 rounded-sm border border-amber-800 bg-amber-900/20 p-3 text-sm text-amber-300">⚠ Hardware detection: {hwErr}</div>}
      {catalogErr && <div className="mb-4 rounded-sm border border-amber-800 bg-amber-900/20 p-3 text-sm text-amber-300">⚠ Model catalog: {catalogErr}</div>}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card title="Engine" icon={<Server size={16} />}>
          <Row label="Status" value={
            engine?.running ? <span className="text-emerald-400">● running</span> : <span className="text-zinc-500">off</span>
          } />
          {engine?.running && (
            <>
              <Row label="Model" value={<span className="font-mono text-xs">{engine.modelPath}</span>} />
              <Row label="Port" value={engine.port ?? "—"} />
              <Row label="Backend" value={engine.backend ?? "—"} />
              <Row label="Speed" value={engine.measuredTps ? `${engine.measuredTps.toFixed(0)} tok/s` : "measuring…"} />
              {engine.embedModel && <Row label="Embed" value={<span className="font-mono text-xs">{engine.embedModel}</span>} />}
            </>
          )}

          <label className="mt-2 flex flex-col gap-1">
            <span className="text-muted">Model</span>
            <select value={settings.selectedModel ?? ""} onChange={(e) => update({ selectedModel: e.target.value || null })}
              className="level-option">
              <option value="">select a model…</option>
              {models.map((m) => <option key={m.id} value={m.defaultFile}>{m.name} · {m.defaultFile}</option>)}
            </select>
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-muted">GPU layers</span>
            <input type="number" value={settings.gpuLayers} onChange={(e) => {
              if (e.target.value === "") update({ gpuLayers: 0 });
              else { const v = Number(e.target.value); if (!Number.isNaN(v)) update({ gpuLayers: v }); }
            }}
              className="input-field font-mono text-xs" />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-muted">Context size</span>
            <input type="number" value={settings.contextSize} onChange={(e) => {
              if (e.target.value === "") update({ contextSize: 8192 });
              else { const v = Number(e.target.value); if (!Number.isNaN(v)) update({ contextSize: v }); }
            }}
              className="input-field font-mono text-xs" />
          </label>

          <div className="mt-2 flex gap-2">
            <button disabled={engBusy || engine?.running} onClick={() => start(settings.selectedModel ?? "", undefined, settings.gpuLayers)}
              className="btn-primary text-xs px-3 py-1.5">
              <Play size={13} /> Start Engine
            </button>
            <button disabled={engBusy || !engine?.running} onClick={stop}
              className="btn-ghost text-xs px-3 py-1.5">
              <Square size={13} /> Stop Engine
            </button>
          </div>
        </Card>

        <Card title="Hardware" icon={<Cpu size={16} />}>
          {hw ? (
            <>
              <Row label="OS / Arch" value={`${hw.os} · ${hw.arch}`} />
              <Row label="CPU" value={`${hw.cpuBrand} (${hw.cpuCores} cores)`} />
              <Row label="RAM" value={fmtBytes(hw.totalRamBytes)} />
              <Row label="GPU" value={`${hw.gpuName ?? "unknown"} · ${fmtBytes(hw.vramBytes)}`} />
              <Row label="GPU backend" value={hw.gpuBackend || "—"} />
            </>
          ) : <span className="text-zinc-500">Detecting…</span>}
        </Card>

        {settings.mode === "team" && (
          <Card title="Team Server" icon={<Server size={16} />}>
            {teamErr && <div className="rounded-sm border border-red-800 bg-red-900/20 px-2 py-1 text-xs text-red-300">{teamErr}</div>}
            <Row label="Team server" value={teamOn ? <span className="text-emerald-400">● on (11436)</span> : <span className="text-zinc-500">off</span>} />

            {!teamOn ? (
              <button disabled={teamBusy} onClick={() => teamStart(11436)}
                className="btn-primary mt-2 text-xs px-3 py-1.5">
                <Server size={13} /> Start Team Server
              </button>
            ) : (
              <>
                {!token ? (
                  <div className="mt-2 flex flex-col gap-2">
                    <input placeholder="username" value={username} onChange={(e) => setUsername(e.target.value)}
                      className="input-field" />
                    <input placeholder="password" type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                      className="input-field" />
                    <div className="flex gap-2">
                      <button onClick={() => login(username, password)}
                        className="btn-primary text-xs px-3 py-1.5">
                        <LogIn size={13} /> Login
                      </button>
                      <button onClick={() => register(username, password)}
                        className="btn-ghost text-xs px-3 py-1.5">
                        <UserPlus size={13} /> Register
                      </button>
                    </div>
                  </div>
                ) : (
                  <>
                    <Row label="Logged in" value={me ?? "—"} />
                    <Row label="Role" value={role ?? "—"} />
                    {role === "admin" && (
                      <div className="mt-2 flex flex-col gap-2">
                        <span className="text-muted">Users</span>
                        {users.map((u) => (
                          <div key={u.id} className="flex items-center justify-between gap-2 rounded-sm border border-[#2A2D45] bg-[#1C1E32] px-2 py-1">
                            <span className="text-[#D0D2E0]">{u.username}</span>
                            <select value={u.role} onChange={(e) => setUserRole(u.id, e.target.value)}
                              className="level-option text-xs">
                              <option value="viewer">viewer</option>
                              <option value="editor">editor</option>
                              <option value="admin">admin</option>
                            </select>
                          </div>
                        ))}
                        {users.length === 0 && <span className="text-muted">no users</span>}
                      </div>
                    )}
                    <button onClick={logout}
                      className="btn-ghost text-xs px-3 py-1.5">
                      <X size={13} /> Log out
                    </button>
                  </>
                )}
                <button disabled={teamBusy} onClick={teamStop}
                  className="btn-ghost mt-2 text-xs px-3 py-1.5">
                  <Square size={13} /> Stop Team Server
                </button>
              </>
            )}
          </Card>
        )}
      </div>
    </div>
  );
}
