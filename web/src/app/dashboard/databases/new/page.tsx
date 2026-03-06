"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import { api, PlanTemplate, AvailableServer } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { toast } from "sonner";
import { Database, ArrowLeft, Package, Shield, ShieldCheck, Check, Server } from "lucide-react";

type DbChoice = "postgresql" | "redis" | "mariadb" | "bundle";

const typeCards: { value: DbChoice; label: string; desc: string; icon: React.ReactNode; gradient: string }[] = [
  {
    value: "postgresql",
    label: "PostgreSQL",
    desc: "Relational database with full SQL support",
    icon: <Database className="h-5 w-5" />,
    gradient: "from-blue-500/20 to-cyan-500/20",
  },
  {
    value: "redis",
    label: "Redis",
    desc: "In-memory key-value store",
    icon: <Database className="h-5 w-5" />,
    gradient: "from-red-500/20 to-orange-500/20",
  },
  {
    value: "mariadb",
    label: "MariaDB",
    desc: "MySQL-compatible relational DB",
    icon: <Database className="h-5 w-5" />,
    gradient: "from-emerald-500/20 to-teal-500/20",
  },
  {
    value: "bundle",
    label: "PG + Redis",
    desc: "Both on a shared network",
    icon: <Package className="h-5 w-5" />,
    gradient: "from-violet-500/20 to-pink-500/20",
  },
];

function formatPrice(cents: number): string {
  return (cents / 100).toFixed(2).replace(/\.?0+$/, "") + "\u20AC";
}

export default function NewDatabasePage() {
  const [name, setName] = useState("");
  const [dbChoice, setDbChoice] = useState<DbChoice>("postgresql");
  const [selectedPlan, setSelectedPlan] = useState<string | null>(null);
  const [sslMode, setSslMode] = useState<"require" | "verify-ca">("require");
  const [loading, setLoading] = useState(false);
  const [plans, setPlans] = useState<PlanTemplate[]>([]);
  const [plansLoading, setPlansLoading] = useState(true);
  const [servers, setServers] = useState<AvailableServer[]>([]);
  const [selectedServer, setSelectedServer] = useState<string>("");
  const { token } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (!token) return;
    api.plans.list(token).then((p) => {
      setPlans(p);
      setPlansLoading(false);
    }).catch(() => setPlansLoading(false));
    api.databases.listServers(token).then(setServers).catch(() => {});
  }, [token]);

  // Filter plans by selected type
  const filteredPlans = plans.filter((p) => {
    if (dbChoice === "bundle") return p.is_bundle;
    return p.db_type.toLowerCase() === dbChoice && !p.is_bundle;
  });

  // Auto-select first plan when type changes
  useEffect(() => {
    if (filteredPlans.length > 0 && !filteredPlans.find((p) => p.id === selectedPlan)) {
      setSelectedPlan(filteredPlans[0].id);
    } else if (filteredPlans.length === 0) {
      setSelectedPlan(null);
    }
  }, [dbChoice, filteredPlans, selectedPlan]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token) return;
    if (!selectedPlan) {
      toast.error("Please select a plan");
      return;
    }
    setLoading(true);
    try {
      const serverOpt = selectedServer ? { server_id: selectedServer } : {};
      if (dbChoice === "bundle") {
        await api.databases.createBundle(token, name, {
          plan_template_id: selectedPlan,
          ssl_mode: sslMode,
          ...serverOpt,
        });
        toast.success("Bundle provisioning started");
      } else {
        await api.databases.create(token, name, dbChoice, {
          plan_template_id: selectedPlan,
          ssl_mode: sslMode,
          ...serverOpt,
        });
        toast.success("Database provisioning started");
      }
      router.push("/dashboard");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create database");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="max-w-lg mx-auto">
      <Link href="/dashboard" className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors mb-6">
        <ArrowLeft className="h-3 w-3" /> Back to databases
      </Link>

      <Card>
        <CardHeader>
          <CardTitle>New Database</CardTitle>
          <CardDescription>Deploy a managed database instance</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-6">
            {/* Name */}
            <div className="space-y-2">
              <Label htmlFor="name">Name</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="my-database"
                pattern="^[a-zA-Z][a-zA-Z0-9_\-]*$"
                className="h-9"
                required
              />
              <p className="text-[11px] text-muted-foreground">Alphanumeric, hyphens, underscores. Must start with a letter.</p>
            </div>

            {/* Type */}
            <div className="space-y-2">
              <Label>Type</Label>
              <div className="grid grid-cols-2 gap-2">
                {typeCards.map((tc) => (
                  <button
                    key={tc.value}
                    type="button"
                    onClick={() => setDbChoice(tc.value)}
                    className={`relative rounded-lg border p-3 text-left transition-all ${
                      dbChoice === tc.value
                        ? "border-primary bg-accent/50 ring-1 ring-primary/50"
                        : "border-border/50 hover:border-border hover:bg-accent/20"
                    }`}
                  >
                    <div className={`h-8 w-8 rounded-md bg-gradient-to-br ${tc.gradient} flex items-center justify-center mb-2`}>
                      {tc.icon}
                    </div>
                    <p className="text-xs font-medium">{tc.label}</p>
                    <p className="text-[10px] text-muted-foreground mt-0.5 leading-tight">{tc.desc}</p>
                  </button>
                ))}
              </div>
              {dbChoice === "bundle" && (
                <p className="text-[11px] text-amber-400">Uses 2 database slots</p>
              )}
            </div>

            {/* Plan Selection */}
            <div className="space-y-2">
              <Label>Plan</Label>
              {plansLoading ? (
                <div className="flex items-center justify-center py-6">
                  <div className="h-5 w-5 animate-spin rounded-full border-2 border-primary border-t-transparent" />
                </div>
              ) : filteredPlans.length === 0 ? (
                <div className="rounded-lg border border-dashed border-border/50 p-4 text-center">
                  <p className="text-xs text-muted-foreground">No plans available for this type</p>
                </div>
              ) : (
                <div className="space-y-2">
                  {filteredPlans.map((plan) => (
                    <button
                      key={plan.id}
                      type="button"
                      onClick={() => setSelectedPlan(plan.id)}
                      className={`w-full rounded-lg border p-3 text-left transition-all ${
                        selectedPlan === plan.id
                          ? "border-primary bg-accent/50 ring-1 ring-primary/50"
                          : "border-border/50 hover:border-border hover:bg-accent/20"
                      }`}
                    >
                      <div className="flex items-center justify-between">
                        <div>
                          <p className="text-sm font-medium">{plan.name}</p>
                          <p className="text-[11px] text-muted-foreground mt-0.5">
                            {plan.cpu_limit} vCPU &middot; {plan.memory_limit_mb} MB RAM
                          </p>
                        </div>
                        <div className="text-right">
                          <p className="text-sm font-semibold">{formatPrice(plan.monthly_price_cents)}/mo</p>
                          <p className="text-[11px] text-muted-foreground">{formatPrice(plan.hourly_price_cents)}/h</p>
                        </div>
                      </div>
                      {selectedPlan === plan.id && (
                        <div className="absolute top-2 right-2">
                          <Check className="h-4 w-4 text-primary" />
                        </div>
                      )}
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Server Selection — only show if multiple servers */}
            {servers.length > 1 && (
              <div className="space-y-2">
                <Label className="text-xs">Server</Label>
                <div className="relative">
                  <Server className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
                  <select
                    value={selectedServer}
                    onChange={(e) => setSelectedServer(e.target.value)}
                    className="w-full h-9 pl-9 pr-3 rounded-md border border-border/50 bg-background text-sm focus:outline-none focus:ring-1 focus:ring-primary/50"
                  >
                    <option value="">Auto (best available)</option>
                    {servers.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.name}{s.region ? ` — ${s.region}` : ""}
                      </option>
                    ))}
                  </select>
                </div>
                <p className="text-[11px] text-muted-foreground">Leave on auto to let the platform choose the best server.</p>
              </div>
            )}

            {/* SSL Mode */}
            <div className="space-y-2">
              <Label className="text-xs">Connection Security</Label>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => setSslMode("require")}
                  className={`flex items-center gap-2 rounded-lg border px-4 py-2.5 text-xs transition-all flex-1 ${
                    sslMode === "require"
                      ? "border-emerald-500/50 bg-emerald-500/10 text-emerald-400"
                      : "border-border/50 hover:border-border text-muted-foreground"
                  }`}
                >
                  <Shield className="h-4 w-4" />
                  <div>
                    <p className="font-medium">Standard</p>
                    <p className="text-[10px] opacity-70">Encrypted, no cert needed</p>
                  </div>
                </button>
                <button
                  type="button"
                  onClick={() => setSslMode("verify-ca")}
                  className={`flex items-center gap-2 rounded-lg border px-4 py-2.5 text-xs transition-all flex-1 ${
                    sslMode === "verify-ca"
                      ? "border-blue-500/50 bg-blue-500/10 text-blue-400"
                      : "border-border/50 hover:border-border text-muted-foreground"
                  }`}
                >
                  <ShieldCheck className="h-4 w-4" />
                  <div>
                    <p className="font-medium">Verified TLS</p>
                    <p className="text-[10px] opacity-70">Encrypted + CA certificate</p>
                  </div>
                </button>
              </div>
              {sslMode === "require" && (
                <p className="text-[11px] text-muted-foreground">Connection is encrypted. Compatible with all clients.</p>
              )}
              {sslMode === "verify-ca" && (
                <p className="text-[11px] text-blue-400">Maximum security. You will need to download the CA certificate after creation.</p>
              )}
            </div>

            <Button type="submit" className="w-full h-10" disabled={loading || !selectedPlan}>
              {loading
                ? "Provisioning..."
                : dbChoice === "bundle"
                ? "Create Bundle"
                : "Create Database"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
