"use client";

import { useEffect, useState, useMemo, useCallback, useRef } from "react";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import { api, API_URL, DatabaseInstance, DbEvent, PlanTemplate, CurrentUsage } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import {
  Plus,
  ExternalLink,
  Trash2,
  Database,
  CircleDot,
  Package,
  Copy,
  CreditCard,
} from "lucide-react";

const statusConfig: Record<string, { color: string; dot: string }> = {
  running: { color: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20", dot: "bg-emerald-400" },
  provisioning: { color: "bg-amber-500/10 text-amber-400 border-amber-500/20", dot: "bg-amber-400 animate-pulse" },
  stopped: { color: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20", dot: "bg-zinc-400" },
  error: { color: "bg-red-500/10 text-red-400 border-red-500/20", dot: "bg-red-400" },
  deleting: { color: "bg-orange-500/10 text-orange-400 border-orange-500/20", dot: "bg-orange-400 animate-pulse" },
};

interface BundleGroup {
  bundleId: string;
  databases: DatabaseInstance[];
}

function StatusBadge({ status }: { status: string }) {
  const config = statusConfig[status] || statusConfig.error;
  return (
    <Badge variant="outline" className={`gap-1.5 ${config.color} border font-normal text-xs`}>
      <span className={`h-1.5 w-1.5 rounded-full ${config.dot}`} />
      {status}
    </Badge>
  );
}

function DbTypeBadge({ type }: { type: string }) {
  return (
    <Badge variant="outline" className="font-normal text-xs gap-1">
      <Database className="h-3 w-3" />
      {type === "postgresql" ? "PostgreSQL" : type === "mariadb" ? "MariaDB" : "Redis"}
    </Badge>
  );
}

function formatPrice(cents: number) {
  return (cents / 100).toFixed(2).replace(/\.00$/, "");
}

function formatLiveCost(cents: number) {
  return (cents / 100).toFixed(4);
}

export default function DashboardPage() {
  const { token } = useAuth();
  const [databases, setDatabases] = useState<DatabaseInstance[]>([]);
  const [plans, setPlans] = useState<PlanTemplate[]>([]);
  const [usage, setUsage] = useState<CurrentUsage | null>(null);
  const [loading, setLoading] = useState(true);
  const [favorites, setFavorites] = useState<string[]>([]);

  const fetchDatabases = useCallback(async () => {
    if (!token) return;
    try {
      const dbs = await api.databases.list(token);
      setDatabases(dbs);
    } catch {
      // silent on poll errors
    } finally {
      setLoading(false);
    }
  }, [token]);

  // Fetch plans once + poll usage every 10s + fetch favorites once
  useEffect(() => {
    if (!token) return;
    api.plans.list(token).then(setPlans).catch(() => {});
    api.databases.listFavorites(token).then(setFavorites).catch(() => {});
    const fetchUsage = () => api.billing.current(token).then(setUsage).catch(() => {});
    fetchUsage();
    const interval = setInterval(fetchUsage, 10000);
    return () => clearInterval(interval);
  }, [token]);

  const planMap = useMemo(() => {
    const m = new Map<string, PlanTemplate>();
    for (const p of plans) m.set(p.id, p);
    return m;
  }, [plans]);

  // Live-interpolated cost counter: ticks every second based on hourly rates
  const [liveCost, setLiveCost] = useState<number | null>(null);

  const totalHourlyCentsPerSecond = useMemo(() => {
    if (!usage || !plans.length) return 0;
    let totalPerHour = 0;
    for (const db of usage.databases) {
      const dbInst = databases.find(d => d.id === db.database_id);
      if (dbInst?.plan_template_id) {
        const plan = planMap.get(dbInst.plan_template_id);
        if (plan) totalPerHour += plan.hourly_price_cents;
      }
    }
    return totalPerHour / 3600; // cents per second
  }, [usage, plans, databases, planMap]);

  useEffect(() => {
    if (!usage) return;
    setLiveCost(usage.total_estimated_cents);
    if (totalHourlyCentsPerSecond <= 0) return;
    const interval = setInterval(() => {
      setLiveCost(prev => prev !== null ? prev + totalHourlyCentsPerSecond : null);
    }, 1000);
    return () => clearInterval(interval);
  }, [usage, totalHourlyCentsPerSecond]);

  // Polling fallback every 5s + SSE for instant updates
  useEffect(() => {
    fetchDatabases();
    const interval = setInterval(fetchDatabases, 5000);
    return () => clearInterval(interval);
  }, [fetchDatabases]);

  useEffect(() => {
    if (!token) return;
    const es = new EventSource(`${API_URL}/api/databases/events?token=${encodeURIComponent(token)}`);

    es.addEventListener("status_changed", () => {
      fetchDatabases();
      api.billing.current(token).then(setUsage).catch(() => {});
    });

    es.addEventListener("deleted", (e) => {
      const event: DbEvent = JSON.parse(e.data);
      setDatabases((prev) => prev.filter((db) => db.id !== event.database_id));
      api.billing.current(token).then(setUsage).catch(() => {});
    });

    es.onerror = () => {};

    return () => es.close();
  }, [token]);

  const handleDelete = async (id: string, name: string) => {
    if (!token || !confirm(`Delete "${name}"? This cannot be undone.`)) return;
    try {
      await api.databases.delete(token, id);
      toast.success("Database deletion initiated");
      fetchDatabases();
    } catch {
      toast.error("Failed to delete database");
    }
  };

  const copyUrl = (url: string) => {
    navigator.clipboard.writeText(url);
    toast.success("Connection URL copied");
  };

  const toggleFavorite = async (id: string) => {
    if (!token) return;
    const isFav = favorites.includes(id);
    try {
      if (isFav) {
        await api.databases.removeFavorite(token, id);
        setFavorites((prev) => prev.filter((f) => f !== id));
      } else {
        await api.databases.addFavorite(token, id);
        setFavorites((prev) => [...prev, id]);
      }
    } catch {
      toast.error("Failed to update favorite");
    }
  };

  const sortedDatabases = useMemo(
    () => [...databases].sort((a, b) => {
      const aFav = favorites.includes(a.id) ? 0 : 1;
      const bFav = favorites.includes(b.id) ? 0 : 1;
      return aFav - bFav;
    }),
    [databases, favorites]
  );

  const { bundles, standalone } = useMemo(() => {
    const bundleMap = new Map<string, DatabaseInstance[]>();
    const standaloneList: DatabaseInstance[] = [];
    for (const db of sortedDatabases) {
      if (db.bundle_id) {
        const existing = bundleMap.get(db.bundle_id) || [];
        existing.push(db);
        bundleMap.set(db.bundle_id, existing);
      } else {
        standaloneList.push(db);
      }
    }
    const bundleGroups: BundleGroup[] = [];
    for (const [bundleId, dbs] of bundleMap) {
      bundleGroups.push({ bundleId, databases: dbs });
    }
    return { bundles: bundleGroups, standalone: standaloneList };
  }, [sortedDatabases]);

  const totalRunning = databases.filter((d) => d.status === "running").length;

  return (
    <div className="space-y-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Databases</h1>
          <p className="text-sm text-muted-foreground mt-1">
            {databases.length} instance{databases.length !== 1 ? "s" : ""} &middot; {totalRunning} running
          </p>
        </div>
        <Button asChild className="gap-2">
          <Link href="/dashboard/databases/new">
            <Plus className="h-4 w-4" /> New Database
          </Link>
        </Button>
      </div>

      {/* Current Month Cost Banner — live counter */}
      {usage && usage.total_estimated_cents > 0 && (
        <Card className="border-primary/20 bg-primary/5">
          <CardContent className="p-4 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <CreditCard className="h-5 w-5 text-primary" />
              <div>
                <p className="text-sm font-medium">Current month estimate</p>
                <p className="text-xs text-muted-foreground">
                  {usage.databases.length} active service{usage.databases.length !== 1 ? "s" : ""} &middot; live
                </p>
              </div>
            </div>
            <div className="text-right">
              <p className="text-lg font-bold font-mono tabular-nums">
                {formatLiveCost(liveCost ?? usage.total_estimated_cents)}€
              </p>
              <Link href="/dashboard/billing" className="text-xs text-primary hover:underline">
                View details
              </Link>
            </div>
          </CardContent>
        </Card>
      )}

      {loading ? (
        <div className="flex items-center justify-center py-20">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
        </div>
      ) : databases.length === 0 ? (
        <Card className="border-dashed">
          <CardContent className="py-16 text-center">
            <Database className="h-10 w-10 text-muted-foreground/50 mx-auto mb-4" />
            <p className="text-muted-foreground mb-1">No databases yet</p>
            <p className="text-xs text-muted-foreground/70 mb-6">Create your first PostgreSQL, Redis, or MariaDB instance</p>
            <Button asChild>
              <Link href="/dashboard/databases/new" className="gap-2">
                <Plus className="h-4 w-4" /> Create Database
              </Link>
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-4">
          {/* Bundles */}
          {bundles.map((bundle) => {
            const bundleName = bundle.databases[0]?.name.replace(/-pg$|-redis$/, "") || "Bundle";
            return (
              <Card key={bundle.bundleId} className="overflow-hidden">
                <div className="px-5 py-3 border-b border-border/50 flex items-center gap-2 bg-accent/30">
                  <Package className="h-4 w-4 text-muted-foreground" />
                  <span className="font-medium text-sm">{bundleName}</span>
                  <Badge variant="outline" className="text-[10px] font-normal">Bundle</Badge>
                </div>
                <div className="divide-y divide-border/30">
                  {bundle.databases.map((db) => {
                    const bPlan = db.plan_template_id ? planMap.get(db.plan_template_id) : null;
                    return (
                    <div key={db.id} className="px-5 py-3 flex items-center justify-between hover:bg-accent/20 transition-colors">
                      <div className="flex items-center gap-3">
                        <button
                          onClick={() => toggleFavorite(db.id)}
                          className="text-base leading-none text-amber-400 hover:scale-110 transition-transform"
                          title={favorites.includes(db.id) ? "Remove from favorites" : "Add to favorites"}
                        >
                          {favorites.includes(db.id) ? "★" : "☆"}
                        </button>
                        <DbTypeBadge type={db.db_type} />
                        <span className="text-sm font-medium">{db.name}</span>
                        <StatusBadge status={db.status} />
                        <span className="text-xs text-muted-foreground font-mono">:{db.port}</span>
                        {bPlan && (
                          <span className="text-[10px] text-muted-foreground">
                            {bPlan.name} · {formatPrice(bPlan.monthly_price_cents)}€/mo
                          </span>
                        )}
                      </div>
                      <div className="flex items-center gap-1">
                        <Button size="sm" variant="ghost" className="h-7 w-7 p-0" onClick={() => copyUrl(db.connection_url)}>
                          <Copy className="h-3.5 w-3.5" />
                        </Button>
                        <Button size="sm" variant="ghost" className="h-7 w-7 p-0" asChild>
                          <Link href={`/dashboard/databases/${db.id}`}><ExternalLink className="h-3.5 w-3.5" /></Link>
                        </Button>
                        <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-destructive hover:text-destructive" onClick={() => handleDelete(db.id, db.name)}>
                          <Trash2 className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    </div>
                    );
                  })}
                </div>
              </Card>
            );
          })}

          {/* Standalone databases */}
          <div className="grid gap-3 md:grid-cols-2">
            {standalone.map((db) => {
              const plan = db.plan_template_id ? planMap.get(db.plan_template_id) : null;
              return (
              <Card key={db.id} className="group hover:border-border transition-colors">
                <CardContent className="p-5 space-y-3">
                  <div className="flex items-start justify-between">
                    <div className="space-y-1">
                      <div className="flex items-center gap-1.5">
                        <button
                          onClick={() => toggleFavorite(db.id)}
                          className="text-base leading-none text-amber-400 hover:scale-110 transition-transform"
                          title={favorites.includes(db.id) ? "Remove from favorites" : "Add to favorites"}
                        >
                          {favorites.includes(db.id) ? "★" : "☆"}
                        </button>
                        <h3 className="font-medium text-sm">{db.name}</h3>
                      </div>
                      <div className="flex items-center gap-2">
                        <DbTypeBadge type={db.db_type} />
                        <StatusBadge status={db.status} />
                        {plan && (
                          <Badge variant="outline" className="text-[10px] font-normal gap-1 text-primary border-primary/20 bg-primary/5">
                            {plan.name} · {formatPrice(plan.monthly_price_cents)}€/mo
                          </Badge>
                        )}
                      </div>
                    </div>
                    <CircleDot className={`h-4 w-4 ${db.status === "running" ? "text-emerald-400" : "text-muted-foreground/30"}`} />
                  </div>
                  <div className="flex items-center gap-1.5">
                    <code className="flex-1 text-[11px] text-muted-foreground font-mono bg-muted/50 px-2.5 py-1.5 rounded truncate">
                      {db.connection_url}
                    </code>
                    <Button size="sm" variant="ghost" className="h-7 w-7 p-0 shrink-0" onClick={() => copyUrl(db.connection_url)}>
                      <Copy className="h-3 w-3" />
                    </Button>
                  </div>
                  <div className="flex items-center gap-2 pt-1">
                    <Button size="sm" variant="outline" className="h-7 text-xs gap-1" asChild>
                      <Link href={`/dashboard/databases/${db.id}`}>
                        <ExternalLink className="h-3 w-3" /> Details
                      </Link>
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-7 text-xs gap-1 text-destructive hover:text-destructive border-destructive/20 hover:bg-destructive/10"
                      onClick={() => handleDelete(db.id, db.name)}
                    >
                      <Trash2 className="h-3 w-3" /> Delete
                    </Button>
                  </div>
                </CardContent>
              </Card>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
