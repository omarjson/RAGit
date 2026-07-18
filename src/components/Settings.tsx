import { useEffect, useState } from "react";
import { Cpu, HardDrive, Server, Users, Play, Square, LogIn, UserPlus, X } from "lucide-react";
import {
  engineStatus,
  detectHardware,
  startEngine,
  stopEngine,
  startTeamServer,
  stopTeamServer,
  teamStatus,
  teamApi,
  listModels,
  HardwareInfo,
  CatalogModel,
  EngineStatus,
  TeamUser,
} from "../lib/ipc";

function fmtBytes(n: number | null | undefined): string {
  if (!n || n === 0) return "unknown";
  return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-zinc-400">{label}</span>
      <span className="text-right text-zinc-200">{value}</span>
    </div>
  );
}

function Card({ title, icon, children }: { title: string; icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
      <div className="mb-3 flex items-center gap-2 text-zinc-100">
        {icon}
        <h2 className="text-sm font-semibold">{title}</h2>
      </div>
      <div className="flex flex-col gap-2 text-sm">{children}</div>
    </div>
  );
}

export function Settings() {
  const [mode, setMode] = useState<"Local" | "Team">(
    () => (localStorage.getItem("mode") as "Local" | "Team") || "Local"
  );
  const [gpuLayers, setGpuLayers] = useState<number>(
    () => Number(localStorage.getItem("gpuLayers") || "32")
  );
  const [contextSize, setContextSize] = useState<number>(
    () => Number(localStorage.getItem("contextSize") || "4096")
  );
  const [selectedModel, setSelectedModel] = useState<string>(
    () => localStorage.getItem("selectedModel") || ""
  );

  const [status, setStatus] = useState<EngineStatus | null>(null);
  const [hw, setHw] = useState<HardwareInfo | null>(null);
  const [models, setModels] = useState<CatalogModel[]>([]);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const [teamOn, setTeamOn] = useState(false);
  const [teamBusy, setTeamBusy] = useState(false);
  const [teamErr, setTeamErr] = useState<string | null>(null);

  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [token, setToken] = useState<string | null>(null);
  const [role, setRole] = useState<string | null>(null);
  const [me, setMe] = useState<string | null>(null);
  const [users, setUsers] = useState<TeamUser[]>([]);

  useEffect(() => {
    localStorage.setItem("mode", mode);
  }, [mode]);
  useEffect(() => {
    localStorage.setItem("gpuLayers", String(gpuLayers));
  }, [gpuLayers]);
  useEffect(() => {
    localStorage.setItem("contextSize", String(contextSize));
  }, [contextSize]);
  useEffect(() => {
    localStorage.setItem("selectedModel", selectedModel);
  }, [selectedModel]);

  const refresh = () => {
    engineStatus().then(setStatus).catch(() => {});
    detectHardware().then(setHw).catch(() => {});
    teamStatus().then(setTeamOn).catch(() => {});
    listModels().then((m) => {
      setModels(m);
      if (!selectedModel && m.length > 0) setSelectedModel(m[0].defaultFile);
    }).catch(() => {});
  };

  useEffect(() => {
    refresh();
  }, []);

  const start = async () => {
    setBusy(true);
    setErr(null);
    try {
      const st = await startEngine(`models/${selectedModel}`, undefined, gpuLayers, undefined, undefined);
      setStatus(st);
    } catch (e) {
      setErr(`⚠ ${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    setErr(null);
    try {
      await stopEngine();
      setStatus({ running: false, modelPath: null, port: null, measuredTps: null, backend: null, embedModel: null, embedPort: null });
    } catch (e) {
      setErr(`⚠ ${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const startTeam = async () => {
    setTeamBusy(true);
    setTeamErr(null);
    try {
      await startTeamServer(11436);
      setTeamOn(true);
    } catch (e) {
      setTeamErr(`⚠ ${String(e)}`);
    } finally {
      setTeamBusy(false);
    }
  };

  const stopTeam = async () => {
    setTeamBusy(true);
    setTeamErr(null);
    try {
      await stopTeamServer();
      setTeamOn(false);
      setToken(null);
      setRole(null);
      setMe(null);
      setUsers([]);
    } catch (e) {
      setTeamErr(`⚠ ${String(e)}`);
    } finally {
      setTeamBusy(false);
    }
  };

  const login = async () => {
    setTeamErr(null);
    try {
      const r = await teamApi("POST", "/api/login", { username, password });
      const data = r as { token: string; role: string };
      setToken(data.token);
      setRole(data.role);
      const meResp = await teamApi("GET", "/api/me", undefined, data.token);
      setMe((meResp as { username: string }).username);
      if (data.role === "admin") {
        const u = await teamApi("GET", "/api/users", undefined, data.token);
        setUsers(u as TeamUser[]);
      }
    } catch (e) {
      setTeamErr(`⚠ ${String(e)}`);
    }
  };

  const register = async () => {
    setTeamErr(null);
    try {
      await teamApi("POST", "/api/register", { username, password });
      await login();
    } catch (e) {
      setTeamErr(`⚠ ${String(e)}`);
    }
  };

  const setUserRole = async (user_id: string, newRole: string) => {
    if (!token) return;
    try {
      await teamApi("POST", "/api/users/role", { user_id, role: newRole }, token);
      setUsers((prev) => prev.map((u) => (u.id === user_id ? { ...u, role: newRole } : u)));
    } catch (e) {
      setTeamErr(`⚠ ${String(e)}`);
    }
  };

  return (
    <div className="p-6">
      <h1 className="mb-1 text-2xl font-semibold">Settings</h1>
      <p className="mb-6 text-sm text-zinc-400">
        Mode (Local / Team), model, GPU layers, context size, hardware, and team management.
      </p>

      <div className="mb-6 flex items-center gap-3 rounded-lg border border-zinc-800 bg-zinc-900/40 p-3 text-sm">
        <span className="text-zinc-400">Mode</span>
        <div className="flex gap-2">
          {(["Local", "Team"] as const).map((m) => (
            <button
              key={m}
              onClick={() => setMode(m)}
              className={`rounded px-3 py-1 font-medium ${
                mode === m ? "bg-brand-fg/80 text-white" : "border border-zinc-700 text-zinc-300 hover:bg-zinc-800"
              }`}
            >
              {m}
            </button>
          ))}
        </div>
      </div>

      {err && <div className="mb-4 rounded-lg border border-red-800 bg-red-900/20 p-3 text-sm text-red-300">{err}</div>}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card title="Engine" icon={<Server size={16} />}>
          <Row
            label="Status"
            value={
              status?.running ? (
                <span className="text-emerald-400">● running</span>
              ) : (
                <span className="text-zinc-500">off</span>
              )
            }
          />
          {status?.running && (
            <>
              <Row label="Model" value={<span className="font-mono text-xs">{status.modelPath}</span>} />
              <Row label="Port" value={status.port ?? "—"} />
              <Row label="Backend" value={status.backend ?? "—"} />
              <Row
                label="Speed"
                value={status.measuredTps ? `${status.measuredTps.toFixed(0)} tok/s` : "measuring…"}
              />
              {status.embedModel && <Row label="Embed" value={<span className="font-mono text-xs">{status.embedModel}</span>} />}
            </>
          )}

          <label className="mt-2 flex flex-col gap-1">
            <span className="text-zinc-400">Model</span>
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value)}
              className="rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-zinc-200"
            >
              <option value="">select a model…</option>
              {models.map((m) => (
                <option key={m.id} value={m.defaultFile}>
                  {m.name} · {m.defaultFile}
                </option>
              ))}
            </select>
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-zinc-400">GPU layers</span>
            <input
              type="number"
              value={gpuLayers}
              onChange={(e) => setGpuLayers(Number(e.target.value))}
              className="rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-zinc-200"
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-zinc-400">Context size</span>
            <input
              type="number"
              value={contextSize}
              onChange={(e) => setContextSize(Number(e.target.value))}
              className="rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-zinc-200"
            />
          </label>

          <div className="mt-2 flex gap-2">
            <button
              disabled={busy || status?.running}
              onClick={start}
              className="flex items-center gap-1 rounded bg-emerald-600/80 px-3 py-1.5 font-medium text-white hover:bg-emerald-600 disabled:opacity-40"
            >
              <Play size={14} /> Start Engine
            </button>
            <button
              disabled={busy || !status?.running}
              onClick={stop}
              className="flex items-center gap-1 rounded border border-zinc-700 px-3 py-1.5 font-medium text-zinc-200 hover:bg-zinc-800 disabled:opacity-40"
            >
              <Square size={14} /> Stop Engine
            </button>
          </div>
        </Card>

        <Card title="Hardware" icon={<Cpu size={16} />}>
          {hw ? (
            <>
              <Row label="OS / Arch" value={`${hw.os} · ${hw.arch}`} />
              <Row label="CPU" value={`${hw.cpuBrand} (${hw.cpuCores} cores)`} />
              <Row label="RAM" value={fmtBytes(hw.totalRamBytes)} />
              <Row
                label="GPU"
                value={
                  <span className="flex items-center gap-1">
                    <HardDrive size={13} className="text-zinc-500" />
                    {hw.gpuName ?? "unknown"} · {fmtBytes(hw.vramBytes)}
                  </span>
                }
              />
              <Row label="GPU backend" value={hw.gpuBackend || "—"} />
            </>
          ) : (
            <span className="text-zinc-500">Detecting…</span>
          )}
        </Card>

        {mode === "Team" && (
          <Card title="Team Server" icon={<Users size={16} />}>
            {teamErr && <div className="rounded border border-red-800 bg-red-900/20 px-2 py-1 text-xs text-red-300">{teamErr}</div>}
            <Row
              label="Team server"
              value={teamOn ? <span className="text-emerald-400">● on (11436)</span> : <span className="text-zinc-500">off</span>}
            />

            {!teamOn ? (
              <button
                disabled={teamBusy}
                onClick={startTeam}
                className="mt-2 flex items-center gap-1 rounded bg-brand-fg/80 px-3 py-1.5 font-medium text-white hover:bg-brand-fg disabled:opacity-40"
              >
                <Server size={14} /> Start Team Server
              </button>
            ) : (
              <>
                {!token ? (
                  <div className="mt-2 flex flex-col gap-2">
                    <input
                      placeholder="username"
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                      className="rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-zinc-200"
                    />
                    <input
                      placeholder="password"
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      className="rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-zinc-200"
                    />
                    <div className="flex gap-2">
                      <button
                        onClick={login}
                        className="flex items-center gap-1 rounded bg-emerald-600/80 px-3 py-1.5 text-white hover:bg-emerald-600"
                      >
                        <LogIn size={14} /> Login
                      </button>
                      <button
                        onClick={register}
                        className="flex items-center gap-1 rounded border border-zinc-700 px-3 py-1.5 text-zinc-200 hover:bg-zinc-800"
                      >
                        <UserPlus size={14} /> Register
                      </button>
                    </div>
                  </div>
                ) : (
                  <>
                    <Row label="Logged in" value={me ?? "—"} />
                    <Row label="Role" value={role ?? "—"} />
                    {role === "admin" && (
                      <div className="mt-2 flex flex-col gap-2">
                        <span className="text-zinc-400">Users</span>
                        {users.map((u) => (
                          <div key={u.id} className="flex items-center justify-between gap-2 rounded border border-zinc-800 bg-zinc-900/50 px-2 py-1">
                            <span className="text-zinc-200">{u.username}</span>
                             <select
                               value={u.role}
                               onChange={(e) => setUserRole(u.id, e.target.value)}
                               className="rounded border border-zinc-700 bg-zinc-900 px-1 py-1 text-xs text-zinc-200"
                             >
                               <option value="viewer">viewer</option>
                               <option value="editor">editor</option>
                               <option value="admin">admin</option>
                             </select>
                          </div>
                        ))}
                        {users.length === 0 && <span className="text-zinc-500">no users</span>}
                      </div>
                    )}
                    <button
                      onClick={() => {
                        setToken(null);
                        setRole(null);
                        setMe(null);
                        setUsers([]);
                      }}
                      className="mt-2 flex items-center gap-1 rounded border border-zinc-700 px-3 py-1.5 text-zinc-200 hover:bg-zinc-800"
                    >
                      <X size={14} /> Log out
                    </button>
                  </>
                )}
                <button
                  disabled={teamBusy}
                  onClick={stopTeam}
                  className="mt-2 flex items-center gap-1 rounded border border-zinc-700 px-3 py-1.5 font-medium text-zinc-200 hover:bg-zinc-800 disabled:opacity-40"
                >
                  <Square size={14} /> Stop Team Server
                </button>
              </>
            )}
          </Card>
        )}
      </div>
    </div>
  );
}
