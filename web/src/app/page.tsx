"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useAuth } from "@/lib/auth";
import { api, PlanTemplate } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Database, Shield, Zap, ArrowRight, Check, Clock, Package, Server } from "lucide-react";

function formatPrice(cents: number) {
  return (cents / 100).toFixed(2).replace(/\.00$/, "");
}

export default function Home() {
  const { user, loading } = useAuth();
  const [plans, setPlans] = useState<PlanTemplate[]>([]);

  useEffect(() => {
    api.plans.listPublic().then(setPlans).catch(() => {});
  }, []);

  const pgPlans = plans.filter((p) => p.db_type.toLowerCase() === "postgresql" && !p.is_bundle);
  const redisPlans = plans.filter((p) => p.db_type.toLowerCase() === "redis" && !p.is_bundle);
  const mariaPlans = plans.filter((p) => p.db_type.toLowerCase() === "mariadb" && !p.is_bundle);
  const bundlePlans = plans.filter((p) => p.is_bundle);

  return (
    <div className="min-h-screen flex flex-col bg-background">
      <nav className="border-b border-border/50 bg-background/80 backdrop-blur-xl">
        <div className="max-w-7xl mx-auto px-6 h-14 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="h-7 w-7 rounded-lg bg-gradient-to-br from-blue-500 to-violet-600 flex items-center justify-center">
              <Database className="h-4 w-4 text-white" />
            </div>
            <span className="font-semibold">DBSaaS</span>
          </div>
          <div className="flex items-center gap-4">
            <a href="#pricing" className="text-sm text-muted-foreground hover:text-foreground transition-colors">
              Pricing
            </a>
            {!loading && (
              <>
                {user ? (
                  <Button size="sm" asChild>
                    <Link href="/dashboard">Dashboard</Link>
                  </Button>
                ) : (
                  <>
                    <Button size="sm" variant="ghost" asChild>
                      <Link href="/login">Login</Link>
                    </Button>
                    <Button size="sm" asChild>
                      <Link href="/register">Get Started</Link>
                    </Button>
                  </>
                )}
              </>
            )}
          </div>
        </div>
      </nav>

      {/* Hero */}
      <div className="flex flex-col items-center justify-center px-6 pt-24 pb-16">
        <div className="max-w-2xl text-center space-y-6">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-border/50 bg-accent/50 text-xs text-muted-foreground">
            <Zap className="h-3 w-3 text-yellow-500" />
            PostgreSQL 17 · Redis 8 · MariaDB 11
          </div>
          <h1 className="text-5xl sm:text-6xl font-bold tracking-tight bg-gradient-to-b from-foreground to-foreground/60 bg-clip-text text-transparent">
            Managed Databases
            <br />
            in Seconds
          </h1>
          <p className="text-lg text-muted-foreground max-w-lg mx-auto">
            Deploy PostgreSQL, Redis &amp; MariaDB instances with TLS encryption, isolated networks, user management, and full admin control.
          </p>
          <div className="flex items-center justify-center gap-3 pt-2">
            {user ? (
              <Button size="lg" asChild>
                <Link href="/dashboard" className="gap-2">
                  Go to Dashboard <ArrowRight className="h-4 w-4" />
                </Link>
              </Button>
            ) : (
              <>
                <Button size="lg" asChild>
                  <Link href="/register" className="gap-2">
                    Get Started <ArrowRight className="h-4 w-4" />
                  </Link>
                </Button>
                <Button size="lg" variant="outline" asChild>
                  <Link href="/login">Login</Link>
                </Button>
              </>
            )}
          </div>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 max-w-3xl w-full mt-20">
          {[
            { icon: Package, title: "Flexible Bundles", desc: "Deploy PG + Redis, PG + MariaDB, or all three together" },
            { icon: Shield, title: "TLS Everywhere", desc: "Auto-generated certificates with internal CA" },
            { icon: Server, title: "Multi-Server", desc: "Your databases run on dedicated Docker servers worldwide" },
          ].map((f) => (
            <div key={f.title} className="group rounded-xl border border-border/50 bg-card/50 p-5 space-y-2 hover:border-border hover:bg-card transition-colors">
              <f.icon className="h-5 w-5 text-muted-foreground group-hover:text-foreground transition-colors" />
              <h3 className="font-medium text-sm">{f.title}</h3>
              <p className="text-xs text-muted-foreground leading-relaxed">{f.desc}</p>
            </div>
          ))}
        </div>
      </div>

      {/* Pricing Section */}
      <div id="pricing" className="px-6 py-20 bg-accent/20 border-t border-border/50">
        <div className="max-w-6xl mx-auto">
          <div className="text-center mb-12">
            <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">Simple, Transparent Pricing</h2>
            <p className="text-muted-foreground mt-3 max-w-lg mx-auto">
              Pay only for what you use. Hourly billing capped at the monthly price — never pay more.
            </p>
          </div>

          {plans.length === 0 ? (
            <div className="text-center text-muted-foreground py-12">
              <p>Plans coming soon — contact us for early access.</p>
            </div>
          ) : (
            <div className="space-y-12">
              {/* PostgreSQL Plans */}
              {pgPlans.length > 0 && (
                <PricingSection
                  icon={<Database className="h-5 w-5 text-blue-400" />}
                  title="PostgreSQL"
                  subtitle="Full SQL relational database"
                  plans={pgPlans}
                  color="blue"
                />
              )}

              {/* Redis Plans */}
              {redisPlans.length > 0 && (
                <PricingSection
                  icon={<Zap className="h-5 w-5 text-red-400" />}
                  title="Redis"
                  subtitle="In-memory key-value store"
                  plans={redisPlans}
                  color="red"
                />
              )}

              {/* MariaDB Plans */}
              {mariaPlans.length > 0 && (
                <PricingSection
                  icon={<Database className="h-5 w-5 text-orange-400" />}
                  title="MariaDB"
                  subtitle="MySQL-compatible relational database"
                  plans={mariaPlans}
                  color="orange"
                />
              )}

              {/* Bundle Plans */}
              {bundlePlans.length > 0 && (
                <PricingSection
                  icon={<Package className="h-5 w-5 text-violet-400" />}
                  title="Bundles"
                  subtitle="PostgreSQL, Redis & MariaDB on a shared private network"
                  plans={bundlePlans}
                  color="violet"
                  isBundle
                />
              )}
            </div>
          )}

          <div className="text-center mt-12">
            <p className="text-sm text-muted-foreground mb-4">
              Need a custom combination? Deploy any mix of PostgreSQL, Redis &amp; MariaDB from your dashboard.
            </p>
            <Button size="lg" asChild>
              <Link href={user ? "/dashboard/databases/new" : "/register"} className="gap-2">
                {user ? "Deploy Now" : "Get Started"} <ArrowRight className="h-4 w-4" />
              </Link>
            </Button>
          </div>
        </div>
      </div>

      {/* Footer */}
      <footer className="border-t border-border/50 py-8 px-6">
        <div className="max-w-5xl mx-auto flex items-center justify-between text-xs text-muted-foreground">
          <span>DBSaaS Platform</span>
          <a href="#pricing" className="hover:text-foreground transition-colors">Pricing</a>
        </div>
      </footer>
    </div>
  );
}

function PricingSection({ icon, title, subtitle, plans, color, isBundle }: {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  plans: PlanTemplate[];
  color: string;
  isBundle?: boolean;
}) {
  return (
    <div>
      <div className="flex items-center gap-2 mb-1">
        {icon}
        <h3 className="text-lg font-semibold">{title}</h3>
      </div>
      <p className="text-xs text-muted-foreground mb-4 ml-7">{subtitle}</p>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {plans.map((plan) => (
          <PricingCard key={plan.id} plan={plan} color={color} isBundle={isBundle} />
        ))}
      </div>
    </div>
  );
}

function PricingCard({ plan, color, isBundle }: { plan: PlanTemplate; color: string; isBundle?: boolean }) {
  const colorMap: Record<string, string> = {
    blue: "hover:border-blue-500/30",
    red: "hover:border-red-500/30",
    orange: "hover:border-orange-500/30",
    violet: "hover:border-violet-500/30",
  };

  const badgeColorMap: Record<string, string> = {
    blue: "bg-blue-500/10 text-blue-400 border-blue-500/20",
    red: "bg-red-500/10 text-red-400 border-red-500/20",
    orange: "bg-orange-500/10 text-orange-400 border-orange-500/20",
    violet: "bg-violet-500/10 text-violet-400 border-violet-500/20",
  };

  return (
    <Card className={`relative overflow-hidden transition-colors ${colorMap[color] || "hover:border-primary/30"}`}>
      <CardContent className="p-6 space-y-4">
        <div className="flex items-start justify-between">
          <div>
            <h4 className="font-semibold">{plan.name}</h4>
            <p className="text-xs text-muted-foreground mt-0.5">
              {plan.cpu_limit} vCPU · {plan.memory_limit_mb} MB RAM
            </p>
          </div>
          {isBundle && (
            <Badge variant="outline" className={`text-[10px] ${badgeColorMap[color]}`}>
              Bundle
            </Badge>
          )}
        </div>

        <div>
          <div className="flex items-baseline gap-1">
            <span className="text-3xl font-bold">{formatPrice(plan.monthly_price_cents)}€</span>
            <span className="text-sm text-muted-foreground">/mo</span>
          </div>
          <div className="flex items-center gap-1.5 mt-1 text-xs text-muted-foreground">
            <Clock className="h-3 w-3" />
            {formatPrice(plan.hourly_price_cents)}€/h — pay only what you use
          </div>
        </div>

        <ul className="space-y-2 text-sm text-muted-foreground">
          <li className="flex items-center gap-2">
            <Check className="h-3.5 w-3.5 text-emerald-400 shrink-0" />
            TLS encryption included
          </li>
          <li className="flex items-center gap-2">
            <Check className="h-3.5 w-3.5 text-emerald-400 shrink-0" />
            Automated backups
          </li>
          <li className="flex items-center gap-2">
            <Check className="h-3.5 w-3.5 text-emerald-400 shrink-0" />
            {isBundle ? "Shared private network" : "Isolated network"}
          </li>
        </ul>
      </CardContent>
    </Card>
  );
}
