"use client";

import { useState, useCallback } from "react";
import { useAuth } from "@/lib/auth";
import { api, AuditLog } from "@/lib/api";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollText, ChevronLeft, ChevronRight, Search } from "lucide-react";
import { useAutoRefresh } from "@/lib/hooks";

export default function AuditPage() {
  const { token } = useAuth();
  const [logs, setLogs] = useState<AuditLog[]>([]);
  const [page, setPage] = useState(1);
  const [filterAction, setFilterAction] = useState("");
  const [filterResource, setFilterResource] = useState("");
  const perPage = 50;

  const load = useCallback(async () => {
    if (!token) return;
    try {
      const data = await api.admin.auditLogs(
        token,
        page,
        perPage,
        filterAction || undefined,
        filterResource || undefined,
      );
      setLogs(data);
    } catch {}
  }, [token, page, filterAction, filterResource]);

  useAutoRefresh(load, 15000);

  const actionColor = (action: string) => {
    if (action.includes("delete") || action.includes("remove")) return "text-red-400 border-red-400/20";
    if (action.includes("create") || action.includes("register")) return "text-emerald-400 border-emerald-400/20";
    if (action.includes("update") || action.includes("scale") || action.includes("rename")) return "text-blue-400 border-blue-400/20";
    if (action.includes("login")) return "text-amber-400 border-amber-400/20";
    return "text-zinc-400 border-zinc-400/20";
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Audit Logs</h1>
          <p className="text-sm text-muted-foreground mt-1">Historique des actions sur la plateforme</p>
        </div>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3">
        <div className="relative flex-1 max-w-xs">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Filtrer par action..."
            value={filterAction}
            onChange={(e) => { setFilterAction(e.target.value); setPage(1); }}
            className="pl-9 h-10"
          />
        </div>
        <select
          className="rounded-md border bg-background px-3 py-2 text-sm h-10"
          value={filterResource}
          onChange={(e) => { setFilterResource(e.target.value); setPage(1); }}
        >
          <option value="">Toutes les ressources</option>
          <option value="database">Database</option>
          <option value="user">User</option>
          <option value="network">Network</option>
          <option value="peering">Peering</option>
          <option value="backup">Backup</option>
          <option value="alert">Alert</option>
        </select>
      </div>

      {/* Logs table */}
      <Card>
        <CardContent className="p-0">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border/30 text-xs text-muted-foreground">
                <th className="text-left px-5 py-3 font-medium">Date</th>
                <th className="text-left px-5 py-3 font-medium">Action</th>
                <th className="text-left px-5 py-3 font-medium">Ressource</th>
                <th className="text-left px-5 py-3 font-medium">User ID</th>
                <th className="text-left px-5 py-3 font-medium">IP</th>
                <th className="text-left px-5 py-3 font-medium">Details</th>
              </tr>
            </thead>
            <tbody>
              {logs.map((log) => (
                <tr key={log.id} className="border-b border-border/10 hover:bg-accent/20 transition-colors">
                  <td className="px-5 py-2.5 text-xs text-muted-foreground whitespace-nowrap">
                    {new Date(log.created_at).toLocaleString("fr")}
                  </td>
                  <td className="px-5 py-2.5">
                    <Badge variant="outline" className={`text-xs font-mono ${actionColor(log.action)}`}>
                      {log.action}
                    </Badge>
                  </td>
                  <td className="px-5 py-2.5">
                    <span className="text-xs">{log.resource_type}</span>
                    {log.resource_id && (
                      <span className="text-[10px] text-muted-foreground ml-1 font-mono">
                        {log.resource_id.slice(0, 8)}...
                      </span>
                    )}
                  </td>
                  <td className="px-5 py-2.5">
                    <span className="text-xs font-mono text-muted-foreground">
                      {log.user_id ? log.user_id.slice(0, 8) + "..." : "—"}
                    </span>
                  </td>
                  <td className="px-5 py-2.5 text-xs text-muted-foreground">
                    {log.ip_address || "—"}
                  </td>
                  <td className="px-5 py-2.5 text-xs text-muted-foreground max-w-[200px] truncate">
                    {log.details ? JSON.stringify(log.details) : "—"}
                  </td>
                </tr>
              ))}
              {logs.length === 0 && (
                <tr>
                  <td colSpan={6} className="text-center py-12 text-muted-foreground">
                    <ScrollText className="h-10 w-10 mx-auto mb-2 opacity-30" />
                    <p>Aucun log</p>
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </CardContent>
      </Card>

      {/* Pagination */}
      <div className="flex items-center justify-between">
        <p className="text-xs text-muted-foreground">{logs.length} resultats — page {page}</p>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" disabled={page <= 1} onClick={() => setPage(page - 1)}>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Button variant="outline" size="sm" disabled={logs.length < perPage} onClick={() => setPage(page + 1)}>
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  );
}
