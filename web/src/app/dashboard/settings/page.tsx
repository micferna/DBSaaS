"use client";

import { useState, useEffect } from "react";
import { useAuth } from "@/lib/auth";
import { api, AuditLog } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import { User, Key, ShieldCheck, Copy, Download, ScrollText } from "lucide-react";

export default function SettingsPage() {
  const { token, user } = useAuth();
  const [apiKey, setApiKey] = useState<string | null>(null);
  const [auditLogs, setAuditLogs] = useState<AuditLog[]>([]);

  useEffect(() => {
    if (!token) return;
    api.audit.list(token).then(setAuditLogs).catch(() => {});
  }, [token]);

  const generateKey = async () => {
    if (!token) return;
    try {
      const res = await api.auth.generateApiKey(token);
      setApiKey(res.api_key);
      toast.success("API key generated");
    } catch {
      toast.error("Failed to generate API key");
    }
  };

  const downloadCaCert = async () => {
    if (!token) return;
    try {
      const cert = await api.databases.getCaCert(token);
      const blob = new Blob([cert], { type: "application/x-pem-file" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "ca.crt";
      a.click();
      URL.revokeObjectURL(url);
    } catch {
      toast.error("Failed to download CA certificate");
    }
  };

  const copyKey = () => {
    if (apiKey) {
      navigator.clipboard.writeText(apiKey);
      toast.success("API key copied");
    }
  };

  return (
    <div className="space-y-6 max-w-2xl">
      <h1 className="text-2xl font-bold tracking-tight">Settings</h1>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <User className="h-4 w-4 text-muted-foreground" /> Account
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-0.5">
              <p className="text-[11px] text-muted-foreground uppercase tracking-wider">Email</p>
              <p className="text-sm">{user?.email}</p>
            </div>
            <div className="space-y-0.5">
              <p className="text-[11px] text-muted-foreground uppercase tracking-wider">Role</p>
              <Badge variant="outline" className="text-xs font-normal">
                {user?.role}
              </Badge>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Key className="h-4 w-4 text-muted-foreground" /> API Key
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-xs text-muted-foreground">Use an API key to access the API programmatically with <code className="text-[11px] bg-muted px-1 py-0.5 rounded">ApiKey &lt;key&gt;</code> header.</p>
          {apiKey && (
            <div className="flex items-center gap-2">
              <code className="flex-1 text-[11px] font-mono bg-muted/50 px-3 py-2 rounded break-all">{apiKey}</code>
              <Button size="sm" variant="ghost" className="h-8 w-8 p-0 shrink-0" onClick={copyKey}>
                <Copy className="h-3.5 w-3.5" />
              </Button>
            </div>
          )}
          <Button onClick={generateKey} variant="outline" size="sm" className="h-8 text-xs gap-1.5">
            <Key className="h-3 w-3" />
            {apiKey ? "Regenerate" : "Generate"} API Key
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <ShieldCheck className="h-4 w-4 text-muted-foreground" /> TLS Certificate
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-xs text-muted-foreground">Download the CA certificate to verify TLS connections to your databases.</p>
          <Button onClick={downloadCaCert} variant="outline" size="sm" className="h-8 text-xs gap-1.5">
            <Download className="h-3 w-3" /> Download CA Certificate
          </Button>
        </CardContent>
      </Card>
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <ScrollText className="h-4 w-4 text-muted-foreground" /> Activity Log
          </CardTitle>
        </CardHeader>
        <CardContent>
          {auditLogs.length === 0 ? (
            <p className="text-xs text-muted-foreground">No recent activity</p>
          ) : (
            <div className="space-y-1 max-h-80 overflow-y-auto">
              {auditLogs.slice(0, 20).map((log) => (
                <div key={log.id} className="flex items-center justify-between text-xs py-1.5 border-b last:border-0 border-border/30">
                  <div className="flex items-center gap-2">
                    <Badge variant="outline" className="text-[10px] font-mono">{log.action}</Badge>
                    <span className="text-muted-foreground">{log.resource_type}</span>
                  </div>
                  <span className="text-muted-foreground whitespace-nowrap">
                    {new Date(log.created_at).toLocaleString()}
                  </span>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
