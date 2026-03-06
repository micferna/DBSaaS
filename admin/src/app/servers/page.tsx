"use client";

import { useState, useCallback } from "react";
import { useAuth } from "@/lib/auth";
import { api, DockerServerStatus } from "@/lib/api";
import { useAutoRefresh } from "@/lib/hooks";
import { useConfirm } from "@/components/ui/confirm-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import { Plus, Trash2, RefreshCw, Server, Cpu, MemoryStick, X, Check, Globe, Wifi, WifiOff, Monitor, Users, Radio, ShieldCheck } from "lucide-react";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";

const fmtMem = (b: number) => {
  const gb = b / (1024 ** 3);
  return gb >= 1 ? `${gb.toFixed(1)} GB` : `${(b / (1024 ** 2)).toFixed(0)} MB`;
};

export default function ServersPage() {
  const { token } = useAuth();
  const [servers, setServers] = useState<DockerServerStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [form, setForm] = useState({ name: "", url: "", region: "", max_containers: 50, notes: "", server_type: "client", tls_ca: "", tls_cert: "", tls_key: "" });
  const { confirm, ConfirmDialog } = useConfirm();

  const load = useCallback(async () => {
    if (!token) return;
    try { setServers(await api.admin.listServers(token)); }
    catch {} finally { setLoading(false); }
  }, [token]);

  const { refreshing, refresh } = useAutoRefresh(load, 5000);

  const add = async () => {
    if (!token || !form.name || !form.url) return;
    try {
      await api.admin.createServer(token, {
        name: form.name,
        url: form.url,
        region: form.region || undefined,
        max_containers: form.max_containers,
        notes: form.notes || undefined,
        server_type: form.server_type,
        tls_ca: form.tls_ca || undefined,
        tls_cert: form.tls_cert || undefined,
        tls_key: form.tls_key || undefined,
      });
      toast.success("Serveur ajoute");
      setForm({ name: "", url: "", region: "", max_containers: 50, notes: "", server_type: "client", tls_ca: "", tls_cert: "", tls_key: "" });
      setShowAdd(false);
      load();
    } catch (err) { toast.error(err instanceof Error ? err.message : "Echec"); }
  };

  const del = async (id: string, name: string) => {
    if (!token) return;
    const ok = await confirm("Retirer le serveur", `Voulez-vous retirer "${name}" ?`);
    if (!ok) return;
    try { await api.admin.deleteServer(token, id); toast.success("Retire"); load(); }
    catch (err) { toast.error(err instanceof Error ? err.message : "Echec"); }
  };

  const toggle = async (id: string, active: boolean) => {
    if (!token) return;
    try { await api.admin.updateServer(token, id, { active: !active }); load(); }
    catch { toast.error("Echec"); }
  };

  const online = servers.filter((s) => s.online);
  const platformServers = servers.filter((s) => s.server_type === "platform");
  const clientServers = servers.filter((s) => s.server_type === "client");
  const totalContainers = online.reduce((a, s) => a + (s.containers_running ?? 0), 0);
  const totalCpu = online.reduce((a, s) => a + (s.cpu_count ?? 0), 0);
  const totalMem = online.reduce((a, s) => a + (s.memory_bytes ?? 0), 0);

  const chartData = servers.filter((s) => s.online).map((s) => ({
    name: s.name, running: s.containers_running ?? 0, max: s.max_containers,
  }));

  const hasTls = (s: DockerServerStatus) => s.url.includes("2376") || s.url.includes("tls");

  return (
    <div className="space-y-8">
      {ConfirmDialog}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Serveurs Docker</h1>
          <p className="text-sm text-muted-foreground mt-1">{online.length}/{servers.length} en ligne · {platformServers.length} platform · {clientServers.length} client</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Radio className={`h-4 w-4 text-emerald-400 ${refreshing ? "animate-pulse" : ""}`} />
            Live — 5s
          </div>
          <Button variant="outline" size="sm" className="gap-1.5 h-10" onClick={refresh} disabled={refreshing}>
            <RefreshCw className={`h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
          </Button>
          <Button size="sm" className="gap-1.5 h-10" onClick={() => setShowAdd(!showAdd)}>
            {showAdd ? <X className="h-4 w-4" /> : <Plus className="h-4 w-4" />}
            {showAdd ? "Annuler" : "Ajouter"}
          </Button>
        </div>
      </div>

      {/* KPIs */}
      <div className="grid gap-4 grid-cols-2 md:grid-cols-4">
        <KPI icon={Server} label="En ligne" value={`${online.length}/${servers.length}`} accent="emerald" />
        <KPI icon={Cpu} label="CPUs total" value={totalCpu} />
        <KPI icon={MemoryStick} label="RAM total" value={totalMem ? fmtMem(totalMem) : "-"} />
        <KPI icon={Server} label="Containers" value={totalContainers} />
      </div>

      {/* Container distribution chart */}
      {chartData.length > 0 && (
        <Card>
          <CardContent className="pt-6">
            <p className="text-sm font-medium text-muted-foreground mb-4">Containers par serveur</p>
            <div className="h-48">
              <ResponsiveContainer>
                <BarChart data={chartData} layout="vertical">
                  <XAxis type="number" tick={{ fontSize: 12 }} stroke="#525252" />
                  <YAxis type="category" dataKey="name" tick={{ fontSize: 12 }} stroke="#525252" width={120} />
                  <Tooltip contentStyle={{ background: "#18181b", border: "1px solid #27272a", borderRadius: 8, fontSize: 13 }} />
                  <Bar dataKey="running" fill="#3b82f6" radius={[0, 4, 4, 0]} name="Running" />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Add form */}
      {showAdd && (
        <Card className="border-primary/20">
          <CardContent className="p-6 space-y-4">
            <p className="text-base font-medium">Ajouter un serveur Docker</p>
            <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
              <div><Label className="text-sm">Nom</Label><Input className="h-10 text-base" placeholder="Paris Node 1" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></div>
              <div><Label className="text-sm">URL Docker</Label><Input className="h-10 text-base" placeholder="tcp://IP:2376 ou local" value={form.url} onChange={(e) => setForm({ ...form, url: e.target.value })} /></div>
              <div><Label className="text-sm">Region</Label><Input className="h-10 text-base" placeholder="eu-west-1" value={form.region} onChange={(e) => setForm({ ...form, region: e.target.value })} /></div>
              <div>
                <Label className="text-sm">Type</Label>
                <select className="w-full h-10 rounded-md border border-input bg-background px-3 text-base" value={form.server_type} onChange={(e) => setForm({ ...form, server_type: e.target.value })}>
                  <option value="client">Client</option>
                  <option value="platform">Platform</option>
                </select>
              </div>
              <div className="flex items-end">
                <Button size="sm" className="w-full h-10 gap-1.5 text-sm" onClick={add} disabled={!form.name || !form.url}>
                  <Check className="h-4 w-4" /> Ajouter
                </Button>
              </div>
            </div>
            {/* TLS fields */}
            <div>
              <p className="text-sm font-medium text-muted-foreground mb-3 flex items-center gap-2">
                <ShieldCheck className="h-4 w-4" /> Certificats TLS (mTLS) — optionnel
              </p>
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div>
                  <Label className="text-sm">CA Certificate</Label>
                  <textarea
                    className="w-full h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono resize-none"
                    placeholder="-----BEGIN CERTIFICATE-----"
                    value={form.tls_ca}
                    onChange={(e) => setForm({ ...form, tls_ca: e.target.value })}
                  />
                </div>
                <div>
                  <Label className="text-sm">Client Certificate</Label>
                  <textarea
                    className="w-full h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono resize-none"
                    placeholder="-----BEGIN CERTIFICATE-----"
                    value={form.tls_cert}
                    onChange={(e) => setForm({ ...form, tls_cert: e.target.value })}
                  />
                </div>
                <div>
                  <Label className="text-sm">Client Key</Label>
                  <textarea
                    className="w-full h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono resize-none"
                    placeholder="-----BEGIN RSA PRIVATE KEY-----"
                    value={form.tls_key}
                    onChange={(e) => setForm({ ...form, tls_key: e.target.value })}
                  />
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Server cards by type */}
      {loading ? (
        <div className="flex justify-center py-16">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
        </div>
      ) : servers.length === 0 ? (
        <Card className="border-dashed">
          <CardContent className="py-12 text-center">
            <Server className="h-10 w-10 text-muted-foreground/40 mx-auto mb-3" />
            <p className="text-base text-muted-foreground">Aucun serveur configure</p>
            <p className="text-sm text-muted-foreground/70 mt-1 mb-4">Ajoutez votre premier serveur Docker</p>
            <Button size="sm" className="h-10" onClick={() => setShowAdd(true)}>Ajouter un serveur</Button>
          </CardContent>
        </Card>
      ) : (
        <>
          {/* Platform servers */}
          {platformServers.length > 0 && (
            <div>
              <div className="flex items-center gap-2 mb-4">
                <Monitor className="h-5 w-5 text-violet-400" />
                <p className="text-base font-semibold">Platform</p>
                <span className="text-xs text-muted-foreground">{platformServers.length} serveurs — API, Admin, Web</span>
              </div>
              <div className="grid gap-4 md:grid-cols-2">
                {platformServers.map((s) => (
                  <ServerCard key={s.id} s={s} onToggle={toggle} onDelete={del} />
                ))}
              </div>
            </div>
          )}

          {/* Client servers */}
          {clientServers.length > 0 && (
            <div>
              <div className="flex items-center gap-2 mb-4">
                <Users className="h-5 w-5 text-blue-400" />
                <p className="text-base font-semibold">Client</p>
                <span className="text-xs text-muted-foreground">{clientServers.length} serveurs — Conteneurs des clients</span>
              </div>
              <div className="grid gap-4 md:grid-cols-2">
                {clientServers.map((s) => (
                  <ServerCard key={s.id} s={s} onToggle={toggle} onDelete={del} />
                ))}
              </div>
            </div>
          )}
        </>
      )}

      <p className="text-xs text-muted-foreground text-center">Rafraichissement auto toutes les 5s</p>
    </div>
  );
}

function ServerCard({ s, onToggle, onDelete }: { s: DockerServerStatus; onToggle: (id: string, active: boolean) => void; onDelete: (id: string, name: string) => void }) {
  return (
    <Card className={`overflow-hidden group transition-all ${s.online ? "hover:border-emerald-500/30" : "hover:border-red-500/30"}`}>
      <CardContent className="p-0">
        <div className="flex items-stretch">
          <div className={`w-1.5 shrink-0 ${s.online ? "bg-emerald-500" : "bg-red-500"}`} />
          <div className="p-5 flex-1 min-w-0">
            <div className="flex items-start justify-between mb-3">
              <div>
                <div className="flex items-center gap-2">
                  <p className="font-semibold text-base">{s.name}</p>
                  {s.online ? (
                    <Badge variant="outline" className="text-xs text-emerald-400 border-emerald-500/20 gap-1"><Wifi className="h-3 w-3" />ON</Badge>
                  ) : (
                    <Badge variant="outline" className="text-xs text-red-400 border-red-500/20 gap-1"><WifiOff className="h-3 w-3" />OFF</Badge>
                  )}
                  {s.region && <Badge variant="outline" className="text-xs gap-1 font-normal"><Globe className="h-3 w-3" />{s.region}</Badge>}
                  <Badge variant="outline" className={`text-xs gap-1 font-normal ${s.server_type === "platform" ? "text-violet-400 border-violet-400/20" : "text-blue-400 border-blue-400/20"}`}>
                    {s.server_type === "platform" ? <Monitor className="h-3 w-3" /> : <Users className="h-3 w-3" />}
                    {s.server_type}
                  </Badge>
                  {(s.url.includes("2376")) && (
                    <Badge variant="outline" className="text-xs gap-1 font-normal text-amber-400 border-amber-400/20">
                      <ShieldCheck className="h-3 w-3" />mTLS
                    </Badge>
                  )}
                </div>
                <p className="text-xs text-muted-foreground font-mono mt-1">{s.url}</p>
                {s.error && <p className="text-xs text-red-400 mt-1">{s.error}</p>}
              </div>
              <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <Button variant="ghost" size="sm" className="h-8 text-xs px-3" onClick={() => onToggle(s.id, s.active)}>
                  {s.active ? "Off" : "On"}
                </Button>
                <Button variant="ghost" size="sm" className="h-8 w-8 p-0 text-destructive" onClick={() => onDelete(s.id, s.name)}>
                  <Trash2 className="h-4 w-4" />
                </Button>
              </div>
            </div>

            {s.online && (
              <div className="grid grid-cols-4 gap-4 pt-3 border-t border-border/20">
                <Stat icon={Server} label="Containers" value={`${s.containers_running}/${s.containers_total}`} />
                <Stat icon={Cpu} label="CPUs" value={s.cpu_count ?? "-"} />
                <Stat icon={MemoryStick} label="RAM" value={s.memory_bytes ? fmtMem(s.memory_bytes) : "-"} />
                <Stat label="Docker" value={s.docker_version ?? "-"} />
              </div>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function KPI({ icon: Icon, label, value, accent }: { icon: React.ElementType; label: string; value: string | number; accent?: string }) {
  const color = accent === "emerald" ? "text-emerald-400" : "text-foreground";
  return (
    <Card><CardContent className="p-5">
      <div className="flex items-center gap-2 mb-2"><Icon className="h-4 w-4 text-muted-foreground" /><span className="text-sm text-muted-foreground">{label}</span></div>
      <p className={`text-3xl font-bold ${color}`}>{value}</p>
    </CardContent></Card>
  );
}

function Stat({ icon: Icon, label, value }: { icon?: React.ElementType; label: string; value: string | number }) {
  return (
    <div>
      <div className="flex items-center gap-1 mb-0.5">{Icon && <Icon className="h-4 w-4 text-muted-foreground" />}<span className="text-xs text-muted-foreground">{label}</span></div>
      <p className="text-sm font-medium">{value}</p>
    </div>
  );
}
