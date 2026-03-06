"use client";

import { useState, useCallback, useEffect } from "react";
import { useAuth } from "@/lib/auth";
import { api, AdminStats, DockerServerStatus, ServerContainer, ServerResources, SystemHealth } from "@/lib/api";
import { useAutoRefresh } from "@/lib/hooks";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { toast } from "sonner";
import { Users, Database, Server, TrendingUp, Lock, Unlock, Radio, Cpu, MemoryStick, HardDrive, Box, ChevronDown, ChevronRight, Wrench, Heart } from "lucide-react";
import {
  AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer,
  PieChart, Pie, Cell, BarChart, Bar,
} from "recharts";

const COLORS = ["#22c55e", "#3b82f6", "#f59e0b", "#ef4444", "#8b5cf6", "#ec4899"];

const fmt = (c: number) => (c / 100).toFixed(2) + "\u20AC";
const fmtMem = (b: number) => {
  const gb = b / (1024 ** 3);
  return gb >= 1 ? `${gb.toFixed(1)} GB` : `${(b / (1024 ** 2)).toFixed(0)} MB`;
};

interface ServerDetail {
  containers: ServerContainer[];
  resources: ServerResources | null;
}

export default function AdminDashboard() {
  const { token } = useAuth();
  const [stats, setStats] = useState<AdminStats | null>(null);
  const [billing, setBilling] = useState<{ total_revenue_cents: number; pending_revenue_cents: number } | null>(null);
  const [servers, setServers] = useState<DockerServerStatus[]>([]);
  const [serverDetails, setServerDetails] = useState<Record<string, ServerDetail>>({});
  const [expandedServers, setExpandedServers] = useState<Set<string>>(new Set());
  const [maintenanceMode, setMaintenanceMode] = useState(false);
  const [health, setHealth] = useState<SystemHealth | null>(null);

  const load = useCallback(async () => {
    if (!token) return;
    const [s, b, srv] = await Promise.all([
      api.admin.stats(token).catch(() => null),
      api.admin.billingOverview(token).catch(() => null),
      api.admin.listServers(token).catch(() => [] as DockerServerStatus[]),
    ]);
    if (s) setStats(s);
    if (b) setBilling(b);
    setServers(srv);

    // Fetch system health
    api.admin.systemHealth(token).then((h) => {
      setHealth(h);
      setMaintenanceMode(h.maintenance_mode);
    }).catch(() => {});

    // Fetch containers + resources for all online servers
    const details: Record<string, ServerDetail> = {};
    await Promise.all(
      srv.filter((s: DockerServerStatus) => s.online).map(async (s: DockerServerStatus) => {
        const [containers, resources] = await Promise.all([
          api.admin.serverContainers(token, s.id).catch(() => [] as ServerContainer[]),
          api.admin.serverResources(token, s.id).catch(() => null),
        ]);
        details[s.id] = { containers, resources };
      })
    );
    setServerDetails(details);
  }, [token]);

  const { refreshing } = useAutoRefresh(load, 5000);

  const toggleMaintenance = async () => {
    if (!token) return;
    try {
      const res = await api.admin.toggleMaintenance(token, !maintenanceMode);
      setMaintenanceMode(res.maintenance_mode);
      toast.success(res.maintenance_mode ? "Mode maintenance active" : "Mode maintenance desactive");
    } catch { toast.error("Echec"); }
  };

  const toggleReg = async () => {
    if (!token || !stats) return;
    try {
      const res = await api.admin.toggleRegistration(token, !stats.registration_enabled);
      setStats({ ...stats, registration_enabled: res.registration_enabled });
      toast.success(res.registration_enabled ? "Inscriptions ouvertes" : "Inscriptions fermees");
    } catch { toast.error("Echec"); }
  };

  const toggleServer = (id: string) => {
    setExpandedServers(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const onlineServers = servers.filter((s) => s.online).length;
  const totalContainers = servers.reduce((a, s) => a + (s.containers_running ?? 0), 0);
  const totalCpu = servers.filter(s => s.online).reduce((a, s) => a + (s.cpu_count ?? 0), 0);
  const totalMem = servers.filter(s => s.online).reduce((a, s) => a + (s.memory_bytes ?? 0), 0);

  // All DBSaaS containers across all servers
  const allContainers = Object.values(serverDetails).flatMap(d => d.containers.filter(c => c.is_dbaas));

  return (
    <div className="space-y-8">
      {/* Live indicator */}
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Radio className={`h-4 w-4 text-emerald-400 ${refreshing ? "animate-pulse" : ""}`} />
          Live — 5s
        </div>
      </div>

      {/* KPI row */}
      <div className="grid gap-4 grid-cols-2 lg:grid-cols-6">
        <KPI icon={Users} label="Users" value={stats?.users ?? 0} />
        <KPI icon={Database} label="Databases" value={stats?.databases ?? 0} />
        <KPI icon={Server} label="Servers" value={`${onlineServers}/${servers.length}`} sub="online" accent={onlineServers > 0 ? "emerald" : "red"} />
        <KPI icon={Cpu} label="CPUs total" value={totalCpu} />
        <KPI icon={TrendingUp} label="Revenue" value={fmt(billing?.total_revenue_cents ?? 0)} accent="emerald" />
        <div className="relative">
          <KPI icon={stats?.registration_enabled ? Unlock : Lock} label="Inscriptions" value={stats?.registration_enabled ? "Open" : "Locked"} accent={stats?.registration_enabled ? "emerald" : "red"} />
          <button
            onClick={toggleReg}
            className="absolute inset-0 opacity-0 hover:opacity-100 bg-background/80 backdrop-blur-sm rounded-xl flex items-center justify-center transition-opacity text-sm font-medium"
          >
            {stats?.registration_enabled ? "Verrouiller" : "Deverrouiller"}
          </button>
        </div>
      </div>

      {/* Maintenance + System Health */}
      <div className="grid gap-4 lg:grid-cols-2">
        <Card className={maintenanceMode ? "border-amber-500/30" : ""}>
          <CardContent className="p-5 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Wrench className={`h-5 w-5 ${maintenanceMode ? "text-amber-400" : "text-muted-foreground"}`} />
              <div>
                <p className="font-medium">Mode Maintenance</p>
                <p className="text-xs text-muted-foreground">Bloque les ecritures pour les non-admins</p>
              </div>
            </div>
            <Button
              size="sm"
              variant={maintenanceMode ? "destructive" : "outline"}
              onClick={toggleMaintenance}
            >
              {maintenanceMode ? "Desactiver" : "Activer"}
            </Button>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-5">
            <div className="flex items-center gap-2 mb-3">
              <Heart className="h-4 w-4 text-muted-foreground" />
              <p className="font-medium">Sante Systeme</p>
            </div>
            {health ? (
              <div className="space-y-2">
                <div className="flex flex-wrap gap-2">
                  {health.servers.map((s) => (
                    <Badge key={s.id} variant="outline" className={`text-xs gap-1.5 ${s.online ? "text-emerald-400 border-emerald-500/20" : "text-red-400 border-red-500/20"}`}>
                      <span className={`h-2 w-2 rounded-full ${s.online ? "bg-emerald-500" : "bg-red-500"}`} />
                      {s.name}
                    </Badge>
                  ))}
                </div>
                <p className="text-xs text-muted-foreground">
                  {health.databases.filter(d => d.status === "running").length}/{health.databases.length} bases actives
                </p>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">Chargement...</p>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Charts row */}
      <div className="grid gap-4 lg:grid-cols-3">
        {/* Growth chart */}
        <Card className="lg:col-span-2">
          <CardContent className="pt-6">
            <p className="text-sm font-medium text-muted-foreground mb-4">Croissance (30j)</p>
            <div className="h-56">
              <ResponsiveContainer>
                <AreaChart data={mergeGrowth(stats?.user_growth, stats?.db_growth)}>
                  <defs>
                    <linearGradient id="gu" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#3b82f6" stopOpacity={0.3} />
                      <stop offset="100%" stopColor="#3b82f6" stopOpacity={0} />
                    </linearGradient>
                    <linearGradient id="gd" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#22c55e" stopOpacity={0.3} />
                      <stop offset="100%" stopColor="#22c55e" stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <XAxis dataKey="date" tick={{ fontSize: 12 }} stroke="#525252" tickFormatter={(v) => v.slice(5)} />
                  <YAxis tick={{ fontSize: 12 }} stroke="#525252" allowDecimals={false} />
                  <Tooltip contentStyle={{ background: "#18181b", border: "1px solid #27272a", borderRadius: 8, fontSize: 13 }} />
                  <Area type="monotone" dataKey="users" stroke="#3b82f6" fill="url(#gu)" strokeWidth={2} name="Users" />
                  <Area type="monotone" dataKey="databases" stroke="#22c55e" fill="url(#gd)" strokeWidth={2} name="Databases" />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </CardContent>
        </Card>

        {/* Type breakdown */}
        <Card>
          <CardContent className="pt-6">
            <p className="text-sm font-medium text-muted-foreground mb-4">Par type</p>
            <div className="h-56 flex items-center justify-center">
              {stats?.type_breakdown && stats.type_breakdown.length > 0 ? (
                <ResponsiveContainer>
                  <PieChart>
                    <Pie data={stats.type_breakdown} dataKey="count" nameKey="type" cx="50%" cy="50%" innerRadius={45} outerRadius={80} paddingAngle={3} strokeWidth={0}>
                      {stats.type_breakdown.map((_, i) => <Cell key={i} fill={COLORS[i % COLORS.length]} />)}
                    </Pie>
                    <Tooltip contentStyle={{ background: "#18181b", border: "1px solid #27272a", borderRadius: 8, fontSize: 13 }} />
                  </PieChart>
                </ResponsiveContainer>
              ) : (
                <p className="text-sm text-muted-foreground">Pas encore de donnees</p>
              )}
            </div>
            <div className="flex flex-wrap gap-2 mt-3 justify-center">
              {stats?.type_breakdown?.map((t, i) => (
                <Badge key={t.type} variant="outline" className="text-xs gap-1.5 font-normal">
                  <span className="h-2.5 w-2.5 rounded-full" style={{ background: COLORS[i % COLORS.length] }} />
                  {t.type} ({t.count})
                </Badge>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Revenue chart */}
      {stats?.revenue_monthly && stats.revenue_monthly.length > 0 && (
        <Card>
          <CardContent className="pt-6">
            <p className="text-sm font-medium text-muted-foreground mb-4">Revenue mensuel</p>
            <div className="h-48">
              <ResponsiveContainer>
                <BarChart data={stats.revenue_monthly}>
                  <XAxis dataKey="month" tick={{ fontSize: 12 }} stroke="#525252" />
                  <YAxis tick={{ fontSize: 12 }} stroke="#525252" tickFormatter={(v) => (v / 100).toFixed(0) + "\u20AC"} />
                  <Tooltip contentStyle={{ background: "#18181b", border: "1px solid #27272a", borderRadius: 8, fontSize: 13 }} formatter={(v) => fmt(v as number)} />
                  <Bar dataKey="total" fill="#22c55e" radius={[4, 4, 0, 0]} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Server resource cards with expandable container list */}
      {servers.length > 0 && (
        <div>
          <p className="text-sm font-medium text-muted-foreground mb-4">Serveurs Docker — Ressources temps reel</p>
          <div className="space-y-3">
            {servers.map((s) => {
              const detail = serverDetails[s.id];
              const res = detail?.resources;
              const containers = detail?.containers || [];
              const expanded = expandedServers.has(s.id);
              const platformContainers = containers.filter(c => c.is_dbaas);

              // Calculate aggregate CPU% for this server
              const totalCpuPct = platformContainers.reduce((a, c) => a + c.cpu_percent, 0);
              const totalMemUsed = platformContainers.reduce((a, c) => a + c.memory_usage_bytes, 0);
              const memTotal = s.memory_bytes ?? 1;
              const memPct = memTotal > 0 ? (totalMemUsed / memTotal) * 100 : 0;

              return (
                <Card key={s.id} className="overflow-hidden">
                  <button
                    onClick={() => toggleServer(s.id)}
                    className="w-full text-left"
                  >
                    <div className="flex items-stretch">
                      <div className={`w-1.5 shrink-0 ${s.online ? "bg-emerald-500" : "bg-red-500"}`} />
                      <div className="p-5 flex-1 min-w-0">
                        <div className="flex items-center justify-between mb-3">
                          <div className="flex items-center gap-2">
                            <Server className="h-5 w-5 text-muted-foreground" />
                            <p className="text-base font-semibold">{s.name}</p>
                            <Badge variant="outline" className={`text-xs ${s.online ? "text-emerald-400 border-emerald-500/20" : "text-red-400 border-red-500/20"}`}>
                              {s.online ? "ON" : "OFF"}
                            </Badge>
                            <Badge variant="outline" className={`text-xs font-normal ${s.server_type === "platform" ? "text-violet-400 border-violet-400/20" : "text-blue-400 border-blue-400/20"}`}>
                              {s.server_type}
                            </Badge>
                            {s.region && <span className="text-xs text-muted-foreground">{s.region}</span>}
                          </div>
                          <div className="flex items-center gap-2">
                            <span className="text-xs text-muted-foreground">{platformContainers.length} containers</span>
                            {expanded ? <ChevronDown className="h-4 w-4 text-muted-foreground" /> : <ChevronRight className="h-4 w-4 text-muted-foreground" />}
                          </div>
                        </div>

                        {s.online && (
                          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                            {/* CPU Gauge */}
                            <div>
                              <div className="flex items-center justify-between mb-1">
                                <span className="text-xs text-muted-foreground flex items-center gap-1"><Cpu className="h-3 w-3" /> CPU</span>
                                <span className="text-xs font-medium">{totalCpuPct.toFixed(1)}%</span>
                              </div>
                              <div className="h-2 bg-accent/50 rounded-full overflow-hidden">
                                <div className={`h-full rounded-full transition-all ${totalCpuPct > 80 ? "bg-red-500" : totalCpuPct > 50 ? "bg-amber-500" : "bg-blue-500"}`} style={{ width: `${Math.min(totalCpuPct, 100)}%` }} />
                              </div>
                              <span className="text-[10px] text-muted-foreground">{s.cpu_count} cores</span>
                            </div>

                            {/* Memory Gauge */}
                            <div>
                              <div className="flex items-center justify-between mb-1">
                                <span className="text-xs text-muted-foreground flex items-center gap-1"><MemoryStick className="h-3 w-3" /> RAM</span>
                                <span className="text-xs font-medium">{memPct.toFixed(1)}%</span>
                              </div>
                              <div className="h-2 bg-accent/50 rounded-full overflow-hidden">
                                <div className={`h-full rounded-full transition-all ${memPct > 80 ? "bg-red-500" : memPct > 50 ? "bg-amber-500" : "bg-violet-500"}`} style={{ width: `${Math.min(memPct, 100)}%` }} />
                              </div>
                              <span className="text-[10px] text-muted-foreground">{fmtMem(totalMemUsed)} / {fmtMem(memTotal)}</span>
                            </div>

                            {/* Containers */}
                            <div>
                              <div className="flex items-center justify-between mb-1">
                                <span className="text-xs text-muted-foreground flex items-center gap-1"><Box className="h-3 w-3" /> Containers</span>
                              </div>
                              <p className="text-lg font-bold">{s.containers_running}<span className="text-xs text-muted-foreground font-normal">/{s.containers_total}</span></p>
                              <span className="text-[10px] text-muted-foreground">max {s.max_containers}</span>
                            </div>

                            {/* Disk */}
                            <div>
                              <div className="flex items-center justify-between mb-1">
                                <span className="text-xs text-muted-foreground flex items-center gap-1"><HardDrive className="h-3 w-3" /> Disk</span>
                              </div>
                              <p className="text-sm font-medium">{res ? fmtMem(res.images_size_bytes + res.containers_size_bytes) : "—"}</p>
                              <span className="text-[10px] text-muted-foreground">{res?.images_count ?? 0} images · {res?.volumes_count ?? 0} volumes</span>
                            </div>
                          </div>
                        )}
                        {s.error && <p className="text-xs text-red-400 mt-2">{s.error}</p>}
                      </div>
                    </div>
                  </button>

                  {/* Expanded container list */}
                  {expanded && platformContainers.length > 0 && (
                    <div className="border-t border-border/30">
                      <table className="w-full text-sm">
                        <thead>
                          <tr className="border-b border-border/20 text-xs text-muted-foreground">
                            <th className="text-left px-5 py-2 font-medium">Container</th>
                            <th className="text-left px-5 py-2 font-medium">Image</th>
                            <th className="text-left px-5 py-2 font-medium">Status</th>
                            <th className="text-left px-5 py-2 font-medium">CPU</th>
                            <th className="text-left px-5 py-2 font-medium">Memoire</th>
                          </tr>
                        </thead>
                        <tbody>
                          {platformContainers.map((c) => {
                            const cMemPct = c.memory_limit_bytes > 0 ? (c.memory_usage_bytes / c.memory_limit_bytes) * 100 : 0;
                            return (
                              <tr key={c.id} className="border-b border-border/10 hover:bg-accent/20 transition-colors">
                                <td className="px-5 py-2">
                                  <span className="font-mono text-xs">{c.name}</span>
                                </td>
                                <td className="px-5 py-2">
                                  <span className="text-xs text-muted-foreground">{c.image}</span>
                                </td>
                                <td className="px-5 py-2">
                                  <Badge variant="outline" className={`text-[10px] ${c.state === "running" ? "text-emerald-400 border-emerald-500/20" : "text-zinc-400 border-zinc-500/20"}`}>
                                    {c.state}
                                  </Badge>
                                </td>
                                <td className="px-5 py-2">
                                  <div className="flex items-center gap-2 min-w-[100px]">
                                    <div className="flex-1 h-1.5 bg-accent/50 rounded-full overflow-hidden">
                                      <div className={`h-full rounded-full ${c.cpu_percent > 80 ? "bg-red-500" : "bg-blue-400"}`} style={{ width: `${Math.min(c.cpu_percent, 100)}%` }} />
                                    </div>
                                    <span className="text-[11px] text-muted-foreground w-12 text-right">{c.cpu_percent.toFixed(1)}%</span>
                                  </div>
                                </td>
                                <td className="px-5 py-2">
                                  <div className="flex items-center gap-2 min-w-[120px]">
                                    <div className="flex-1 h-1.5 bg-accent/50 rounded-full overflow-hidden">
                                      <div className={`h-full rounded-full ${cMemPct > 80 ? "bg-red-500" : "bg-violet-400"}`} style={{ width: `${Math.min(cMemPct, 100)}%` }} />
                                    </div>
                                    <span className="text-[11px] text-muted-foreground w-16 text-right">{fmtMem(c.memory_usage_bytes)}</span>
                                  </div>
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  )}
                  {expanded && platformContainers.length === 0 && (
                    <div className="border-t border-border/30 px-5 py-4">
                      <p className="text-xs text-muted-foreground text-center">Aucun container DBSaaS sur ce serveur</p>
                    </div>
                  )}
                </Card>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

function KPI({ icon: Icon, label, value, sub, accent }: {
  icon: React.ElementType; label: string; value: string | number; sub?: string; accent?: string;
}) {
  const color = accent === "emerald" ? "text-emerald-400" : accent === "red" ? "text-red-400" : "text-foreground";
  return (
    <Card>
      <CardContent className="p-5">
        <div className="flex items-center gap-2 mb-2">
          <Icon className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm text-muted-foreground font-medium">{label}</span>
        </div>
        <p className={`text-3xl font-bold ${color}`}>{value}</p>
        {sub && <p className="text-xs text-muted-foreground mt-1">{sub}</p>}
      </CardContent>
    </Card>
  );
}

function mergeGrowth(
  users?: Array<{ date: string; count: number }>,
  dbs?: Array<{ date: string; count: number }>,
) {
  const map = new Map<string, { date: string; users: number; databases: number }>();
  for (const u of users ?? []) {
    map.set(u.date, { date: u.date, users: u.count, databases: 0 });
  }
  for (const d of dbs ?? []) {
    const existing = map.get(d.date) ?? { date: d.date, users: 0, databases: 0 };
    existing.databases = d.count;
    map.set(d.date, existing);
  }
  return Array.from(map.values()).sort((a, b) => a.date.localeCompare(b.date));
}
