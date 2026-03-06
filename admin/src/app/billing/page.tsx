"use client";

import { useState, useCallback } from "react";
import { useAuth } from "@/lib/auth";
import { api, BillingPeriod } from "@/lib/api";
import { useAutoRefresh } from "@/lib/hooks";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { toast } from "sonner";
import { RefreshCw, TrendingUp, Clock, AlertCircle, CheckCircle, Radio } from "lucide-react";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";

const fmt = (c: number) => (c / 100).toFixed(2) + "\u20AC";
const STATUS_STYLE: Record<string, { bg: string; icon: React.ElementType }> = {
  paid:     { bg: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20", icon: CheckCircle },
  invoiced: { bg: "bg-blue-500/10 text-blue-400 border-blue-500/20", icon: Clock },
  pending:  { bg: "bg-amber-500/10 text-amber-400 border-amber-500/20", icon: Clock },
  failed:   { bg: "bg-red-500/10 text-red-400 border-red-500/20", icon: AlertCircle },
};

export default function BillingPage() {
  const { token } = useAuth();
  const [overview, setOverview] = useState<{
    total_revenue_cents: number; pending_revenue_cents: number; total_periods: number; periods: BillingPeriod[];
  } | null>(null);
  const [generating, setGenerating] = useState(false);

  const load = useCallback(async () => {
    if (!token) return;
    try { setOverview(await api.admin.billingOverview(token)); } catch {}
  }, [token]);

  const { refreshing } = useAutoRefresh(load, 60000);

  const generate = async () => {
    if (!token) return;
    setGenerating(true);
    try {
      const res = await api.admin.generateBilling(token);
      toast.success(`${res.invoices_created} factures generees`);
      load();
    } catch (err) { toast.error(err instanceof Error ? err.message : "Echec"); }
    finally { setGenerating(false); }
  };

  const monthlyData = overview?.periods
    ? Object.entries(
        overview.periods.reduce((acc, p) => {
          const m = p.period_start.slice(0, 7);
          acc[m] = (acc[m] || 0) + p.total_cents;
          return acc;
        }, {} as Record<string, number>)
      ).map(([month, total]) => ({ month, total })).sort((a, b) => a.month.localeCompare(b.month))
    : [];

  return (
    <div className="space-y-8">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Facturation</h1>
          <p className="text-sm text-muted-foreground mt-1">{overview?.total_periods ?? 0} periodes</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Radio className={`h-4 w-4 text-emerald-400 ${refreshing ? "animate-pulse" : ""}`} />
            Live — 60s
          </div>
          <Button size="sm" className="gap-1.5 h-10" onClick={generate} disabled={generating}>
            <RefreshCw className={`h-4 w-4 ${generating ? "animate-spin" : ""}`} />
            Generer factures du mois
          </Button>
        </div>
      </div>

      {overview && (
        <>
          {/* KPIs */}
          <div className="grid gap-4 grid-cols-3">
            <Card>
              <CardContent className="p-5">
                <div className="flex items-center gap-2 mb-2">
                  <TrendingUp className="h-4 w-4 text-emerald-400" />
                  <span className="text-sm text-muted-foreground">Revenue total</span>
                </div>
                <p className="text-3xl font-bold text-emerald-400">{fmt(overview.total_revenue_cents)}</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="p-5">
                <div className="flex items-center gap-2 mb-2">
                  <Clock className="h-4 w-4 text-amber-400" />
                  <span className="text-sm text-muted-foreground">En attente</span>
                </div>
                <p className="text-3xl font-bold text-amber-400">{fmt(overview.pending_revenue_cents)}</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="p-5">
                <div className="flex items-center gap-2 mb-2">
                  <CheckCircle className="h-4 w-4 text-muted-foreground" />
                  <span className="text-sm text-muted-foreground">Periodes</span>
                </div>
                <p className="text-3xl font-bold">{overview.total_periods}</p>
              </CardContent>
            </Card>
          </div>

          {/* Revenue chart */}
          {monthlyData.length > 0 && (
            <Card>
              <CardContent className="pt-6">
                <p className="text-sm font-medium text-muted-foreground mb-4">Revenue par mois</p>
                <div className="h-48">
                  <ResponsiveContainer>
                    <BarChart data={monthlyData}>
                      <XAxis dataKey="month" tick={{ fontSize: 12 }} stroke="#525252" />
                      <YAxis tick={{ fontSize: 12 }} stroke="#525252" tickFormatter={(v) => fmt(v)} />
                      <Tooltip contentStyle={{ background: "#18181b", border: "1px solid #27272a", borderRadius: 8, fontSize: 13 }} formatter={(v) => fmt(v as number)} />
                      <Bar dataKey="total" fill="#22c55e" radius={[4, 4, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              </CardContent>
            </Card>
          )}

          {/* Periods table */}
          {overview.periods.length > 0 && (
            <Card>
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b border-border/50 text-sm text-muted-foreground">
                      <th className="text-left px-5 py-3 font-medium">User</th>
                      <th className="text-left px-5 py-3 font-medium">Periode</th>
                      <th className="text-left px-5 py-3 font-medium">Status</th>
                      <th className="text-left px-5 py-3 font-medium">Stripe</th>
                      <th className="text-right px-5 py-3 font-medium">Montant</th>
                    </tr>
                  </thead>
                  <tbody>
                    {overview.periods.map((p) => {
                      const st = STATUS_STYLE[p.status] || STATUS_STYLE.pending;
                      return (
                        <tr key={p.id} className="border-b border-border/20 hover:bg-accent/30 transition-colors">
                          <td className="px-5 py-3">
                            <code className="text-sm font-mono text-muted-foreground">{p.user_id.slice(0, 8)}...</code>
                          </td>
                          <td className="px-5 py-3 text-sm text-muted-foreground">
                            {new Date(p.period_start).toLocaleDateString("fr")} — {new Date(p.period_end).toLocaleDateString("fr")}
                          </td>
                          <td className="px-5 py-3">
                            <Badge variant="outline" className={`text-xs gap-1.5 ${st.bg}`}>
                              {p.status}
                            </Badge>
                          </td>
                          <td className="px-5 py-3">
                            {p.stripe_invoice_id ? (
                              <code className="text-xs font-mono text-muted-foreground">{p.stripe_invoice_id.slice(0, 16)}...</code>
                            ) : <span className="text-xs text-muted-foreground">-</span>}
                          </td>
                          <td className="px-5 py-3 text-right font-semibold text-base">{fmt(p.total_cents)}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </Card>
          )}

          {overview.periods.length === 0 && (
            <Card className="border-dashed">
              <CardContent className="py-12 text-center">
                <p className="text-muted-foreground text-base">Aucune facture encore</p>
                <p className="text-sm text-muted-foreground/70 mt-1">Cliquez sur &quot;Generer&quot; pour facturer le mois precedent</p>
              </CardContent>
            </Card>
          )}
        </>
      )}
    </div>
  );
}
