"use client";

import { useState, useCallback } from "react";
import { useAuth } from "@/lib/auth";
import { api, PlanTemplate } from "@/lib/api";
import { useAutoRefresh } from "@/lib/hooks";
import { useConfirm } from "@/components/ui/confirm-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import { Plus, Pencil, Trash2, X, Check, Cpu, MemoryStick, Database, Zap, Package, Radio } from "lucide-react";

const DB_TYPES = ["postgresql", "redis", "mariadb"] as const;
const TYPE_META: Record<string, { icon: React.ElementType; color: string; label: string }> = {
  postgresql: { icon: Database, color: "text-blue-400", label: "PostgreSQL" },
  redis:      { icon: Zap,      color: "text-red-400",  label: "Redis" },
  mariadb:    { icon: Database, color: "text-orange-400", label: "MariaDB" },
};

const fmt = (c: number) => (c / 100).toFixed(2).replace(/\.00$/, "") + "\u20AC";

export default function PlansPage() {
  const { token } = useAuth();
  const [plans, setPlans] = useState<PlanTemplate[]>([]);
  const [editId, setEditId] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const emptyForm = { name: "", db_type: "postgresql" as "postgresql" | "redis" | "mariadb", cpu_limit: 0.5, memory_limit_mb: 256, monthly_price_cents: 500, hourly_price_cents: 2, is_bundle: false, active: true };
  const [form, setForm] = useState(emptyForm);
  const { confirm, ConfirmDialog } = useConfirm();

  const load = useCallback(async () => {
    if (!token) return;
    try { setPlans(await api.admin.listPlans(token)); } catch {}
  }, [token]);

  const { refreshing } = useAutoRefresh(load, 30000);

  const close = () => { setShowForm(false); setEditId(null); setForm(emptyForm); };
  const edit = (p: PlanTemplate) => {
    setForm({ name: p.name, db_type: p.db_type, cpu_limit: p.cpu_limit, memory_limit_mb: p.memory_limit_mb, monthly_price_cents: p.monthly_price_cents, hourly_price_cents: p.hourly_price_cents, is_bundle: p.is_bundle, active: p.active });
    setEditId(p.id); setShowForm(true);
  };
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token) return;
    try {
      if (editId) { await api.admin.updatePlan(token, editId, form); toast.success("Plan mis a jour"); }
      else { await api.admin.createPlan(token, form); toast.success("Plan cree"); }
      close(); await load();
    } catch (err) { toast.error(err instanceof Error ? err.message : "Echec"); }
  };
  const del = async (id: string) => {
    if (!token) return;
    const ok = await confirm("Desactiver le plan", "Voulez-vous desactiver ce plan ?");
    if (!ok) return;
    try { await api.admin.deletePlan(token, id); toast.success("Desactive"); await load(); }
    catch { toast.error("Echec"); }
  };

  const groups = [
    { key: "postgresql", plans: plans.filter((p) => p.db_type === "postgresql" && !p.is_bundle) },
    { key: "redis", plans: plans.filter((p) => p.db_type === "redis" && !p.is_bundle) },
    { key: "mariadb", plans: plans.filter((p) => p.db_type === "mariadb" && !p.is_bundle) },
    { key: "bundle", plans: plans.filter((p) => p.is_bundle) },
  ];

  return (
    <div className="space-y-8">
      {ConfirmDialog}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Plans</h1>
          <p className="text-sm text-muted-foreground mt-1">{plans.filter((p) => p.active).length} actifs / {plans.length} total</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Radio className={`h-4 w-4 text-emerald-400 ${refreshing ? "animate-pulse" : ""}`} />
            Live — 30s
          </div>
          <Button size="sm" className="gap-1.5 h-10" onClick={() => { close(); setShowForm(true); }}>
            <Plus className="h-4 w-4" /> Nouveau plan
          </Button>
        </div>
      </div>

      {showForm && (
        <Card className="border-primary/20">
          <CardContent className="p-6">
            <form onSubmit={submit} className="space-y-4">
              <div className="flex items-center justify-between mb-1">
                <p className="text-base font-medium">{editId ? "Modifier" : "Creer"} un plan</p>
                <Button type="button" variant="ghost" size="sm" className="h-8 w-8 p-0" onClick={close}><X className="h-4 w-4" /></Button>
              </div>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <div><Label className="text-sm">Nom</Label><Input className="h-10 text-base" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} required /></div>
                <div><Label className="text-sm">Type</Label>
                  <select className="w-full h-10 rounded-md border border-input bg-background px-3 text-base" value={form.db_type} onChange={(e) => setForm({ ...form, db_type: e.target.value as typeof form.db_type })}>
                    {DB_TYPES.map((t) => <option key={t} value={t}>{TYPE_META[t]?.label ?? t}</option>)}
                  </select>
                </div>
                <div><Label className="text-sm">CPU</Label><Input className="h-10 text-base" type="number" step="0.25" min="0.25" value={form.cpu_limit} onChange={(e) => setForm({ ...form, cpu_limit: parseFloat(e.target.value) })} /></div>
                <div><Label className="text-sm">RAM (MB)</Label><Input className="h-10 text-base" type="number" step="64" min="64" value={form.memory_limit_mb} onChange={(e) => setForm({ ...form, memory_limit_mb: parseInt(e.target.value) })} /></div>
              </div>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <div><Label className="text-sm">Prix/mois (cents)</Label><Input className="h-10 text-base" type="number" min="0" value={form.monthly_price_cents} onChange={(e) => setForm({ ...form, monthly_price_cents: parseInt(e.target.value) })} /><span className="text-xs text-muted-foreground">{fmt(form.monthly_price_cents)}/mo</span></div>
                <div><Label className="text-sm">Prix/h (cents)</Label><Input className="h-10 text-base" type="number" min="0" value={form.hourly_price_cents} onChange={(e) => setForm({ ...form, hourly_price_cents: parseInt(e.target.value) })} /><span className="text-xs text-muted-foreground">{fmt(form.hourly_price_cents)}/h</span></div>
                <div className="flex items-end gap-4 pb-1">
                  <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={form.is_bundle} onChange={(e) => setForm({ ...form, is_bundle: e.target.checked })} className="rounded" />Bundle</label>
                  <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={form.active} onChange={(e) => setForm({ ...form, active: e.target.checked })} className="rounded" />Actif</label>
                </div>
                <div className="flex items-end"><Button type="submit" size="sm" className="w-full h-10 gap-1.5 text-sm"><Check className="h-4 w-4" />{editId ? "Sauver" : "Creer"}</Button></div>
              </div>
            </form>
          </CardContent>
        </Card>
      )}

      {groups.map((g) => {
        if (g.plans.length === 0) return null;
        const meta = g.key === "bundle" ? { icon: Package, color: "text-violet-400", label: "Bundles (PG + Redis + MariaDB)" } : TYPE_META[g.key];
        const Icon = meta.icon;
        return (
          <div key={g.key}>
            <div className="flex items-center gap-2 mb-4">
              <Icon className={`h-5 w-5 ${meta.color}`} />
              <p className="text-base font-semibold">{meta.label}</p>
              <span className="text-xs text-muted-foreground">{g.plans.length}</span>
            </div>
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
              {g.plans.map((p) => (
                <Card key={p.id} className={`group transition-all ${!p.active ? "opacity-40" : "hover:border-primary/30"}`}>
                  <CardContent className="p-5">
                    <div className="flex items-start justify-between mb-3">
                      <div>
                        <p className="font-semibold text-base">{p.name}</p>
                        <div className="flex items-center gap-3 mt-1 text-xs text-muted-foreground">
                          <span className="flex items-center gap-1"><Cpu className="h-3.5 w-3.5" />{p.cpu_limit} vCPU</span>
                          <span className="flex items-center gap-1"><MemoryStick className="h-3.5 w-3.5" />{p.memory_limit_mb} MB</span>
                        </div>
                      </div>
                      <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0" onClick={() => edit(p)}><Pencil className="h-4 w-4" /></Button>
                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0 text-destructive" onClick={() => del(p.id)}><Trash2 className="h-4 w-4" /></Button>
                      </div>
                    </div>
                    <div className="flex items-baseline gap-1">
                      <span className="text-2xl font-bold">{fmt(p.monthly_price_cents)}</span>
                      <span className="text-sm text-muted-foreground">/mois</span>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1">{fmt(p.hourly_price_cents)}/h — plafonne au mensuel</p>
                    <div className="flex gap-2 mt-3">
                      {!p.active && <Badge variant="outline" className="text-xs text-red-400 border-red-400/20">Inactif</Badge>}
                      {p.is_bundle && <Badge variant="outline" className="text-xs text-violet-400 border-violet-400/20">Bundle</Badge>}
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
