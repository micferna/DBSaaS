"use client";

import { useEffect, useState, useCallback, useMemo } from "react";
import { useAuth } from "@/lib/auth";
import { api, BillingPeriod, CurrentUsage, PlanTemplate, DatabaseInstance } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Receipt, TrendingUp, Clock, Database, Activity, Zap } from "lucide-react";

function formatCents(cents: number): string {
  return (cents / 100).toFixed(2) + "\u20AC";
}

function formatCentsLive(cents: number): string {
  return (cents / 100).toFixed(4) + "\u20AC";
}

const statusColors: Record<string, string> = {
  pending: "bg-amber-500/10 text-amber-400 border-amber-500/20",
  invoiced: "bg-blue-500/10 text-blue-400 border-blue-500/20",
  paid: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
  failed: "bg-red-500/10 text-red-400 border-red-500/20",
};

export default function BillingPage() {
  const { token } = useAuth();
  const [periods, setPeriods] = useState<BillingPeriod[]>([]);
  const [currentUsage, setCurrentUsage] = useState<CurrentUsage | null>(null);
  const [plans, setPlans] = useState<PlanTemplate[]>([]);
  const [databases, setDatabases] = useState<DatabaseInstance[]>([]);
  const [loading, setLoading] = useState(true);
  const [liveCost, setLiveCost] = useState<number | null>(null);
  const [liveDbCosts, setLiveDbCosts] = useState<Record<string, number>>({});

  const fetchData = useCallback(async () => {
    if (!token) return;
    try {
      const [p, c] = await Promise.all([
        api.billing.periods(token).catch(() => [] as BillingPeriod[]),
        api.billing.current(token).catch(() => null),
      ]);
      setPeriods(p);
      if (c) setCurrentUsage(c);
    } catch {
      // silent
    } finally {
      setLoading(false);
    }
  }, [token]);

  // Initial + live polling every 10s
  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 10000);
    return () => clearInterval(interval);
  }, [fetchData]);

  // Load plans + databases once for hourly rate calculation
  useEffect(() => {
    if (!token) return;
    api.plans.list(token).then(setPlans).catch(() => {});
    api.databases.list(token).then(setDatabases).catch(() => {});
  }, [token]);

  const planMap = useMemo(() => {
    const m = new Map<string, PlanTemplate>();
    for (const p of plans) m.set(p.id, p);
    return m;
  }, [plans]);

  // Per-database hourly rates (cents/sec)
  const dbRates = useMemo(() => {
    const rates: Record<string, number> = {};
    if (!currentUsage || !plans.length) return rates;
    for (const db of currentUsage.databases) {
      const dbInst = databases.find(d => d.id === db.database_id);
      if (dbInst?.plan_template_id) {
        const plan = planMap.get(dbInst.plan_template_id);
        if (plan) rates[db.database_id] = plan.hourly_price_cents / 3600;
      }
    }
    return rates;
  }, [currentUsage, plans, databases, planMap]);

  const totalCentsPerSecond = useMemo(() =>
    Object.values(dbRates).reduce((a, b) => a + b, 0),
  [dbRates]);

  // Reset live costs when API data arrives, then tick every second
  useEffect(() => {
    if (!currentUsage) return;
    setLiveCost(currentUsage.total_estimated_cents);
    const initCosts: Record<string, number> = {};
    for (const db of currentUsage.databases) {
      initCosts[db.database_id] = db.estimated_cents;
    }
    setLiveDbCosts(initCosts);

    if (totalCentsPerSecond <= 0) return;
    const interval = setInterval(() => {
      setLiveCost(prev => prev !== null ? prev + totalCentsPerSecond : null);
      setLiveDbCosts(prev => {
        const next = { ...prev };
        for (const [id, rate] of Object.entries(dbRates)) {
          next[id] = (next[id] ?? 0) + rate;
        }
        return next;
      });
    }, 1000);
    return () => clearInterval(interval);
  }, [currentUsage, totalCentsPerSecond, dbRates]);

  const activeCount = currentUsage?.databases.filter(d => d.hours_used > 0).length ?? 0;
  const totalHours = currentUsage?.databases.reduce((sum, d) => sum + d.hours_used, 0) ?? 0;

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Billing</h1>
        <p className="text-sm text-muted-foreground mt-1">Usage tracking and invoices</p>
      </div>

      {/* KPI Cards */}
      <div className="grid grid-cols-3 gap-4">
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center gap-3">
              <div className="h-10 w-10 rounded-lg bg-emerald-500/10 flex items-center justify-center">
                <TrendingUp className="h-5 w-5 text-emerald-400" />
              </div>
              <div>
                <p className="text-2xl font-bold font-mono tabular-nums">{formatCentsLive(liveCost ?? currentUsage?.total_estimated_cents ?? 0)}</p>
                <p className="text-[11px] text-muted-foreground">Current month estimate &middot; live</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center gap-3">
              <div className="h-10 w-10 rounded-lg bg-blue-500/10 flex items-center justify-center">
                <Activity className="h-5 w-5 text-blue-400" />
              </div>
              <div>
                <p className="text-2xl font-bold">{activeCount}</p>
                <p className="text-[11px] text-muted-foreground">Active service{activeCount !== 1 ? "s" : ""}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center gap-3">
              <div className="h-10 w-10 rounded-lg bg-violet-500/10 flex items-center justify-center">
                <Zap className="h-5 w-5 text-violet-400" />
              </div>
              <div>
                <p className="text-2xl font-bold font-mono tabular-nums">{totalHours.toFixed(2)}h</p>
                <p className="text-[11px] text-muted-foreground">Total compute hours &middot; live</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Current Usage Detail */}
      {currentUsage && currentUsage.databases.length > 0 && (
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <CardTitle className="text-base flex items-center gap-2">
                <Database className="h-4 w-4" />
                Active Services
              </CardTitle>
              <div className="flex items-center gap-1.5 text-[10px] text-emerald-400">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" /> Live — 10s
              </div>
            </div>
            <CardDescription>Real-time cost tracking for running databases</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-1">
              {currentUsage.databases.map((db) => {
                const pct = currentUsage.total_estimated_cents > 0
                  ? (db.estimated_cents / currentUsage.total_estimated_cents) * 100
                  : 0;
                return (
                  <div key={db.database_id} className="flex items-center gap-4 py-3 border-b border-border/30 last:border-0">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <Database className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                        <p className="text-sm font-medium truncate">{db.database_name}</p>
                        {db.plan_name && (
                          <Badge variant="outline" className="text-[10px] font-normal shrink-0">
                            {db.plan_name}
                          </Badge>
                        )}
                      </div>
                      <div className="mt-1.5 flex items-center gap-2">
                        <div className="flex-1 h-1.5 rounded-full bg-muted/50 overflow-hidden">
                          <div
                            className="h-full rounded-full bg-primary transition-all duration-500"
                            style={{ width: `${Math.min(pct, 100)}%` }}
                          />
                        </div>
                        <span className="text-[10px] text-muted-foreground w-10 text-right">{pct.toFixed(0)}%</span>
                      </div>
                    </div>
                    <div className="text-right shrink-0">
                      <p className="text-sm font-semibold font-mono tabular-nums">{formatCentsLive(liveDbCosts[db.database_id] ?? db.estimated_cents)}</p>
                      <p className="text-[10px] text-muted-foreground">{db.hours_used.toFixed(1)}h used</p>
                    </div>
                  </div>
                );
              })}
            </div>
          </CardContent>
        </Card>
      )}

      {/* No active services */}
      {currentUsage && currentUsage.databases.length === 0 && (
        <Card>
          <CardContent className="py-10 text-center">
            <Database className="h-8 w-8 text-muted-foreground/50 mx-auto mb-2" />
            <p className="text-sm text-muted-foreground">No active databases this month</p>
          </CardContent>
        </Card>
      )}

      {/* Billing History */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base flex items-center gap-2">
            <Receipt className="h-4 w-4" />
            Billing History
          </CardTitle>
        </CardHeader>
        <CardContent>
          {periods.length === 0 ? (
            <div className="py-8 text-center">
              <Clock className="h-8 w-8 text-muted-foreground/50 mx-auto mb-2" />
              <p className="text-sm text-muted-foreground">No billing periods yet</p>
            </div>
          ) : (
            <div className="space-y-2">
              {periods.map((period) => (
                <div key={period.id} className="flex items-center justify-between py-3 border-b border-border/30 last:border-0">
                  <div>
                    <p className="text-sm font-medium">
                      {new Date(period.period_start).toLocaleDateString("en-US", { month: "long", year: "numeric" })}
                    </p>
                    <p className="text-[11px] text-muted-foreground">
                      {new Date(period.period_start).toLocaleDateString()} - {new Date(period.period_end).toLocaleDateString()}
                    </p>
                  </div>
                  <div className="flex items-center gap-3">
                    <Badge variant="outline" className={`text-[10px] ${statusColors[period.status] || ""}`}>
                      {period.status}
                    </Badge>
                    <p className="text-sm font-semibold w-20 text-right">{formatCents(period.total_cents)}</p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
