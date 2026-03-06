"use client";

import { useState, useCallback, useMemo } from "react";
import { useAuth } from "@/lib/auth";
import { api, AdminDatabase, PlanTemplate } from "@/lib/api";
import { useAutoRefresh } from "@/lib/hooks";
import { useConfirm } from "@/components/ui/confirm-dialog";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { toast } from "sonner";
import { Trash2, Database, Zap, Search, Radio, Server, Package, ChevronDown, ChevronRight, User, ArrowUpRight } from "lucide-react";
import { Input } from "@/components/ui/input";

const STATUS: Record<string, { bg: string; dot: string }> = {
  running:      { bg: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20", dot: "bg-emerald-400" },
  provisioning: { bg: "bg-amber-500/10 text-amber-400 border-amber-500/20", dot: "bg-amber-400 animate-pulse" },
  stopped:      { bg: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20", dot: "bg-zinc-400" },
  error:        { bg: "bg-red-500/10 text-red-400 border-red-500/20", dot: "bg-red-400" },
  deleting:     { bg: "bg-orange-500/10 text-orange-400 border-orange-500/20", dot: "bg-orange-400 animate-pulse" },
};

const TYPE_CONFIG: Record<string, { icon: React.ElementType; color: string; border: string; label: string }> = {
  postgresql: { icon: Database, color: "text-blue-400", border: "border-l-blue-500", label: "PostgreSQL" },
  redis:      { icon: Zap,      color: "text-red-400",  border: "border-l-red-500",  label: "Redis" },
  mariadb:    { icon: Database, color: "text-orange-400", border: "border-l-orange-500", label: "MariaDB" },
};

const fmt = (c: number) => (c / 100).toFixed(2).replace(/\.00$/, "") + "\u20AC";

interface BundleGroup {
  bundle_id: string;
  databases: AdminDatabase[];
}

export default function DatabasesPage() {
  const { token } = useAuth();
  const [databases, setDatabases] = useState<AdminDatabase[]>([]);
  const [plans, setPlans] = useState<PlanTemplate[]>([]);
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [expandedBundles, setExpandedBundles] = useState<Set<string>>(new Set());
  const { confirm, ConfirmDialog } = useConfirm();

  const load = useCallback(async () => {
    if (!token) return;
    try {
      const [dbs, p] = await Promise.all([api.admin.listDatabases(token), api.admin.listPlans(token)]);
      setDatabases(dbs);
      setPlans(p);
    } catch {}
  }, [token]);

  const { refreshing } = useAutoRefresh(load, 10000);

  const planMap = useMemo(() => {
    const m = new Map<string, PlanTemplate>();
    for (const p of plans) m.set(p.id, p);
    return m;
  }, [plans]);

  const del = async (id: string, name: string) => {
    if (!token) return;
    const ok = await confirm("Force delete", `Supprimer definitivement "${name}" ?`);
    if (!ok) return;
    try { await api.admin.forceDeleteDatabase(token, id); toast.success("Supprime"); load(); }
    catch { toast.error("Echec"); }
  };

  const migrateSni = async (id: string, name: string) => {
    if (!token) return;
    const ok = await confirm("Migrer vers SNI", `Migrer "${name}" vers le routage SNI ?`);
    if (!ok) return;
    try { await api.admin.migrateSni(token, id); toast.success("Migration SNI reussie"); load(); }
    catch { toast.error("Echec de la migration SNI"); }
  };

  const filtered = useMemo(() => {
    return databases.filter((d) => {
      if (statusFilter !== "all" && d.status !== statusFilter) return false;
      if (search) {
        const q = search.toLowerCase();
        if (!d.name.toLowerCase().includes(q) && !d.user_email.toLowerCase().includes(q) && !d.server_name.toLowerCase().includes(q)) return false;
      }
      return true;
    });
  }, [databases, search, statusFilter]);

  // Group by bundle_id
  const { bundles, standalone } = useMemo(() => {
    const bundleMap = new Map<string, AdminDatabase[]>();
    const standalone: AdminDatabase[] = [];

    for (const db of filtered) {
      if (db.bundle_id) {
        const existing = bundleMap.get(db.bundle_id) || [];
        existing.push(db);
        bundleMap.set(db.bundle_id, existing);
      } else {
        standalone.push(db);
      }
    }

    const bundles: BundleGroup[] = [];
    for (const [bundle_id, dbs] of bundleMap) {
      bundles.push({ bundle_id, databases: dbs });
    }

    return { bundles, standalone };
  }, [filtered]);

  const toggleBundle = (id: string) => {
    setExpandedBundles(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const statusCounts = useMemo(() => {
    const m: Record<string, number> = {};
    for (const d of databases) m[d.status] = (m[d.status] || 0) + 1;
    return m;
  }, [databases]);

  return (
    <div className="space-y-8">
      {ConfirmDialog}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Bases de donnees</h1>
          <p className="text-sm text-muted-foreground mt-1">{databases.length} instances · {bundles.length} bundles</p>
        </div>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Radio className={`h-4 w-4 text-emerald-400 ${refreshing ? "animate-pulse" : ""}`} />
          Live — 10s
        </div>
      </div>

      {/* Filter bar */}
      <div className="flex items-center gap-3 flex-wrap">
        <div className="relative flex-1 max-w-xs">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input className="h-10 pl-9 text-base" placeholder="Rechercher nom, email, serveur..." value={search} onChange={(e) => setSearch(e.target.value)} />
        </div>
        <div className="flex gap-1">
          <FilterBtn active={statusFilter === "all"} onClick={() => setStatusFilter("all")}>Tous ({databases.length})</FilterBtn>
          {Object.entries(statusCounts).map(([s, c]) => (
            <FilterBtn key={s} active={statusFilter === s} onClick={() => setStatusFilter(s)}>
              <span className={`h-2 w-2 rounded-full ${STATUS[s]?.dot || "bg-zinc-400"}`} />
              {s} ({c})
            </FilterBtn>
          ))}
        </div>
      </div>

      {/* Bundles */}
      {bundles.map((bundle) => {
        const expanded = expandedBundles.has(bundle.bundle_id);
        const firstDb = bundle.databases[0];
        return (
          <Card key={bundle.bundle_id} className="overflow-hidden">
            <button
              onClick={() => toggleBundle(bundle.bundle_id)}
              className="w-full flex items-center gap-3 px-5 py-4 hover:bg-accent/30 transition-colors text-left"
            >
              <div className="h-9 w-9 rounded-lg bg-violet-500/10 flex items-center justify-center">
                <Package className="h-5 w-5 text-violet-400" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-semibold text-base">Bundle</span>
                  <Badge variant="outline" className="text-xs bg-violet-500/10 text-violet-400 border-violet-500/20">
                    {bundle.databases.length} DBs
                  </Badge>
                </div>
                <div className="flex items-center gap-3 text-xs text-muted-foreground mt-0.5">
                  <span className="flex items-center gap-1"><User className="h-3 w-3" />{firstDb?.user_email || "—"}</span>
                  <span className="flex items-center gap-1"><Server className="h-3 w-3" />{firstDb?.server_name || "Local"}</span>
                </div>
              </div>
              {expanded ? <ChevronDown className="h-5 w-5 text-muted-foreground" /> : <ChevronRight className="h-5 w-5 text-muted-foreground" />}
            </button>
            {expanded && (
              <div className="border-t border-border/30 overflow-x-auto">
                <table className="w-full text-sm">
                  <tbody>
                    {bundle.databases.map((db) => (
                      <DbRow key={db.id} db={db} planMap={planMap} onDelete={del} onMigrateSni={migrateSni} />
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </Card>
        );
      })}

      {/* Standalone databases */}
      {standalone.length > 0 && (
        <Card>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border/50 text-sm text-muted-foreground">
                  <th className="text-left px-5 py-3 font-medium">Nom</th>
                  <th className="text-left px-5 py-3 font-medium">Type</th>
                  <th className="text-left px-5 py-3 font-medium">Status</th>
                  <th className="text-left px-5 py-3 font-medium">Utilisateur</th>
                  <th className="text-left px-5 py-3 font-medium">Serveur</th>
                  <th className="text-left px-5 py-3 font-medium">Plan</th>
                  <th className="text-left px-5 py-3 font-medium">Ressources</th>
                  <th className="text-right px-5 py-3 font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                {standalone.map((db) => (
                  <DbRow key={db.id} db={db} planMap={planMap} onDelete={del} onMigrateSni={migrateSni} />
                ))}
              </tbody>
            </table>
            {standalone.length === 0 && (
              <p className="text-base text-muted-foreground text-center py-10">Aucune base standalone</p>
            )}
          </div>
        </Card>
      )}

      {filtered.length === 0 && bundles.length === 0 && (
        <p className="text-base text-muted-foreground text-center py-10">Aucune base trouvee</p>
      )}
    </div>
  );
}

function DbRow({ db, planMap, onDelete, onMigrateSni }: { db: AdminDatabase; planMap: Map<string, PlanTemplate>; onDelete: (id: string, name: string) => void; onMigrateSni: (id: string, name: string) => void }) {
  const t = TYPE_CONFIG[db.db_type] || TYPE_CONFIG.postgresql;
  const s = STATUS[db.status] || STATUS.error;
  const plan = db.plan_template_id ? planMap.get(db.plan_template_id) : null;
  const isSni = db.routing_mode === "sni";

  // Resource bar helper
  const maxCpu = 4;
  const maxRam = 2048;
  const cpuPct = Math.min((db.cpu_limit / maxCpu) * 100, 100);
  const ramPct = Math.min((db.memory_limit_mb / maxRam) * 100, 100);

  return (
    <tr className={`border-b border-border/20 hover:bg-accent/30 transition-colors border-l-2 ${t.border}`}>
      <td className="px-5 py-3">
        <p className="font-medium text-base">{db.name}</p>
        {isSni && db.subdomain ? (
          <p className="text-xs text-muted-foreground font-mono">{db.subdomain}</p>
        ) : (
          <p className="text-xs text-muted-foreground font-mono">:{db.port}</p>
        )}
      </td>
      <td className="px-5 py-3">
        <div className="flex items-center gap-2">
          <t.icon className={`h-4 w-4 ${t.color}`} />
          <span className="text-sm">{t.label}</span>
        </div>
      </td>
      <td className="px-5 py-3">
        <Badge variant="outline" className={`text-xs gap-1.5 ${s.bg}`}>
          <span className={`h-2 w-2 rounded-full ${s.dot}`} />
          {db.status}
        </Badge>
      </td>
      <td className="px-5 py-3">
        <div className="flex items-center gap-1.5">
          <User className="h-3.5 w-3.5 text-muted-foreground" />
          <span className="text-sm truncate max-w-[160px]" title={db.user_email}>{db.user_email || "—"}</span>
        </div>
      </td>
      <td className="px-5 py-3">
        <div className="flex items-center gap-1.5">
          <Server className="h-3.5 w-3.5 text-muted-foreground" />
          <span className="text-sm">{db.server_name}</span>
        </div>
      </td>
      <td className="px-5 py-3">
        {plan ? (
          <span className="text-sm">{plan.name} <span className="text-muted-foreground">({fmt(plan.monthly_price_cents)}/mo)</span></span>
        ) : (
          <span className="text-xs text-muted-foreground">—</span>
        )}
      </td>
      <td className="px-5 py-3">
        <div className="space-y-1.5 min-w-[120px]">
          <div className="flex items-center gap-2">
            <span className="text-[11px] text-muted-foreground w-8">CPU</span>
            <div className="flex-1 h-1.5 bg-accent/50 rounded-full overflow-hidden">
              <div className="h-full bg-blue-400 rounded-full transition-all" style={{ width: `${cpuPct}%` }} />
            </div>
            <span className="text-[11px] text-muted-foreground w-12 text-right">{db.cpu_limit}v</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[11px] text-muted-foreground w-8">RAM</span>
            <div className="flex-1 h-1.5 bg-accent/50 rounded-full overflow-hidden">
              <div className="h-full bg-violet-400 rounded-full transition-all" style={{ width: `${ramPct}%` }} />
            </div>
            <span className="text-[11px] text-muted-foreground w-12 text-right">{db.memory_limit_mb}M</span>
          </div>
        </div>
      </td>
      <td className="px-5 py-3 text-right">
        <div className="flex items-center justify-end gap-1">
          {!isSni && (
            <Button variant="ghost" size="sm" className="h-8 gap-1 text-xs text-blue-400" onClick={() => onMigrateSni(db.id, db.name)} title="Migrer vers SNI">
              <ArrowUpRight className="h-3.5 w-3.5" /> SNI
            </Button>
          )}
          {isSni && (
            <Badge variant="outline" className="text-[10px] bg-blue-500/10 text-blue-400 border-blue-500/20">SNI</Badge>
          )}
          <Button variant="ghost" size="sm" className="h-8 w-8 p-0 text-destructive" onClick={() => onDelete(db.id, db.name)}>
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      </td>
    </tr>
  );
}

function FilterBtn({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors ${
        active ? "bg-primary text-primary-foreground" : "bg-accent/50 text-muted-foreground hover:bg-accent"
      }`}
    >
      {children}
    </button>
  );
}
