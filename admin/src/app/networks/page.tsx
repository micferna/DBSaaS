"use client";

import { Fragment, useState, useCallback } from "react";
import { useAuth } from "@/lib/auth";
import { api } from "@/lib/api";
import { useAutoRefresh } from "@/lib/hooks";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Network,
  ChevronDown,
  ChevronRight,
  Database,
  Radio,
} from "lucide-react";

type AdminNetwork = Awaited<ReturnType<typeof api.admin.listNetworks>>[number];

const dbTypeColor = (t: string) => {
  switch (t) {
    case "postgresql": return "bg-blue-500/10 text-blue-600";
    case "redis": return "bg-red-500/10 text-red-600";
    case "mariadb": return "bg-emerald-500/10 text-emerald-600";
    default: return "";
  }
};

const dbTypePort = (t: string) => {
  switch (t) {
    case "postgresql": return 5432;
    case "redis": return 6379;
    case "mariadb": return 3306;
    default: return 0;
  }
};

export default function AdminNetworksPage() {
  const { token } = useAuth();
  const [networks, setNetworks] = useState<AdminNetwork[]>([]);
  const [loading, setLoading] = useState(true);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const loadNetworks = useCallback(async () => {
    if (!token) return;
    try {
      setNetworks(await api.admin.listNetworks(token));
    } catch {} finally {
      setLoading(false);
    }
  }, [token]);

  const { refreshing } = useAutoRefresh(loadNetworks, 15000);

  const toggleExpand = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Reseaux prives</h1>
          <p className="text-sm text-muted-foreground mt-1">{networks.length} reseau{networks.length !== 1 ? "x" : ""}</p>
        </div>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Radio className={`h-4 w-4 text-emerald-400 ${refreshing ? "animate-pulse" : ""}`} />
          Live — 15s
        </div>
      </div>

      {networks.length === 0 ? (
        <Card>
          <CardContent className="py-12 text-center">
            <Network className="h-10 w-10 mx-auto text-muted-foreground mb-3" />
            <p className="text-base text-muted-foreground">Aucun reseau prive</p>
          </CardContent>
        </Card>
      ) : (
        <div className="rounded-lg border">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b bg-muted/50 text-sm text-muted-foreground">
                <th className="text-left px-5 py-3 font-medium"></th>
                <th className="text-left px-5 py-3 font-medium">Nom</th>
                <th className="text-left px-5 py-3 font-medium">Sous-reseau</th>
                <th className="text-left px-5 py-3 font-medium">Proprietaire</th>
                <th className="text-left px-5 py-3 font-medium">Membres</th>
                <th className="text-left px-5 py-3 font-medium">Cree le</th>
              </tr>
            </thead>
            <tbody>
              {networks.map((net) => {
                const isExpanded = expanded.has(net.id);
                return (
                  <Fragment key={net.id}>
                    <tr
                      className="border-b hover:bg-muted/30 cursor-pointer"
                      onClick={() => toggleExpand(net.id)}
                    >
                      <td className="px-5 py-3">
                        {isExpanded ? (
                          <ChevronDown className="h-4 w-4 text-muted-foreground" />
                        ) : (
                          <ChevronRight className="h-4 w-4 text-muted-foreground" />
                        )}
                      </td>
                      <td className="px-5 py-3 font-medium text-base">{net.name}</td>
                      <td className="px-5 py-3">
                        {net.subnet ? (
                          <code className="text-xs font-mono bg-muted px-1.5 py-0.5 rounded">{net.subnet}</code>
                        ) : (
                          <span className="text-xs text-muted-foreground">Non attribue</span>
                        )}
                      </td>
                      <td className="px-5 py-3 font-mono text-sm text-muted-foreground">
                        {net.user_id.slice(0, 8)}...
                      </td>
                      <td className="px-5 py-3">
                        <Badge variant="secondary">{net.member_count}</Badge>
                      </td>
                      <td className="px-5 py-3 text-sm text-muted-foreground">
                        {new Date(net.created_at).toLocaleDateString("fr-FR")}
                      </td>
                    </tr>
                    {isExpanded && net.members.length > 0 && (
                      <tr key={`${net.id}-details`}>
                        <td colSpan={6} className="px-10 py-4 bg-muted/20">
                          {net.gateway && (
                            <p className="text-xs text-muted-foreground mb-3">
                              Passerelle : <code className="bg-muted px-1.5 py-0.5 rounded font-mono">{net.gateway}</code>
                            </p>
                          )}
                          <div className="space-y-2">
                            {net.members.map((m) => (
                              <div
                                key={m.database_id}
                                className="flex items-center gap-3 text-sm"
                              >
                                <Database className="h-4 w-4 text-muted-foreground" />
                                <span className="font-medium">{m.database_name}</span>
                                <Badge
                                  variant="secondary"
                                  className={`text-xs ${dbTypeColor(m.db_type)}`}
                                >
                                  {m.db_type}
                                </Badge>
                                <code className="bg-muted px-2 py-0.5 rounded font-mono text-sm">
                                  {m.hostname}:{dbTypePort(m.db_type)}
                                </code>
                              </div>
                            ))}
                          </div>
                        </td>
                      </tr>
                    )}
                  </Fragment>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
