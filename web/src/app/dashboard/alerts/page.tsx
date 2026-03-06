"use client";

import { useEffect, useState } from "react";
import { useAuth } from "@/lib/auth";
import { api, AlertRule, AlertHistory, DatabaseInstance } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Bell, Plus, Trash2, History } from "lucide-react";

export default function AlertsPage() {
  const { token } = useAuth();
  const [rules, setRules] = useState<AlertRule[]>([]);
  const [history, setHistory] = useState<AlertHistory[]>([]);
  const [databases, setDatabases] = useState<DatabaseInstance[]>([]);
  const [showCreate, setShowCreate] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [loading, setLoading] = useState(true);

  // Create form
  const [newEventType, setNewEventType] = useState("db_down");
  const [newDatabaseId, setNewDatabaseId] = useState("");
  const [newWebhookUrl, setNewWebhookUrl] = useState("");
  const [newEmail, setNewEmail] = useState("");

  useEffect(() => {
    if (!token) return;
    loadData();
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [token]);

  async function loadData() {
    if (!token) return;
    try {
      const [r, d] = await Promise.all([
        api.alerts.list(token),
        api.databases.list(token),
      ]);
      setRules(r);
      setDatabases(d);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }

  async function loadHistory() {
    if (!token) return;
    const h = await api.alerts.history(token);
    setHistory(h);
    setShowHistory(true);
  }

  async function createRule() {
    if (!token) return;
    if (!newWebhookUrl && !newEmail) return alert("Webhook URL or email required");
    try {
      await api.alerts.create(token, {
        database_id: newDatabaseId || undefined,
        event_type: newEventType,
        webhook_url: newWebhookUrl || undefined,
        email: newEmail || undefined,
      });
      setShowCreate(false);
      setNewWebhookUrl("");
      setNewEmail("");
      loadData();
    } catch (e: unknown) {
      alert((e as Error).message);
    }
  }

  async function toggleRule(id: string, enabled: boolean) {
    if (!token) return;
    await api.alerts.update(token, id, { enabled: !enabled });
    loadData();
  }

  async function deleteRule(id: string) {
    if (!token) return;
    await api.alerts.delete(token, id);
    loadData();
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Bell className="h-6 w-6 text-primary" />
          <h1 className="text-2xl font-bold">Alerts</h1>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={loadHistory}>
            <History className="h-4 w-4 mr-1" /> History
          </Button>
          <Button size="sm" onClick={() => setShowCreate(!showCreate)}>
            <Plus className="h-4 w-4 mr-1" /> New Alert
          </Button>
        </div>
      </div>

      {showCreate && (
        <div className="border rounded-lg p-4 space-y-3 bg-card">
          <h3 className="font-semibold">Create Alert Rule</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className="text-sm text-muted-foreground">Event Type</label>
              <select
                className="w-full mt-1 rounded-md border bg-background px-3 py-2 text-sm"
                value={newEventType}
                onChange={(e) => setNewEventType(e.target.value)}
              >
                <option value="db_down">Database Down</option>
                <option value="db_error">Database Error</option>
                <option value="backup_failed">Backup Failed</option>
                <option value="high_cpu">High CPU (&gt;90%)</option>
                <option value="high_memory">High Memory (&gt;90%)</option>
              </select>
            </div>
            <div>
              <label className="text-sm text-muted-foreground">Database (optional)</label>
              <select
                className="w-full mt-1 rounded-md border bg-background px-3 py-2 text-sm"
                value={newDatabaseId}
                onChange={(e) => setNewDatabaseId(e.target.value)}
              >
                <option value="">All databases</option>
                {databases.map((db) => (
                  <option key={db.id} value={db.id}>{db.name}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-sm text-muted-foreground">Webhook URL</label>
              <Input
                placeholder="https://hooks.example.com/..."
                value={newWebhookUrl}
                onChange={(e) => setNewWebhookUrl(e.target.value)}
                className="mt-1"
              />
            </div>
            <div>
              <label className="text-sm text-muted-foreground">Email</label>
              <Input
                placeholder="alerts@example.com"
                value={newEmail}
                onChange={(e) => setNewEmail(e.target.value)}
                className="mt-1"
              />
            </div>
          </div>
          <div className="flex gap-2 pt-2">
            <Button size="sm" onClick={createRule}>Create</Button>
            <Button size="sm" variant="ghost" onClick={() => setShowCreate(false)}>Cancel</Button>
          </div>
        </div>
      )}

      {rules.length === 0 ? (
        <div className="text-center py-12 text-muted-foreground">
          <Bell className="h-12 w-12 mx-auto mb-3 opacity-30" />
          <p>No alert rules configured</p>
          <p className="text-sm mt-1">Create alerts to get notified about database events</p>
        </div>
      ) : (
        <div className="space-y-2">
          {rules.map((rule) => {
            const dbName = databases.find((d) => d.id === rule.database_id)?.name || "All databases";
            return (
              <div key={rule.id} className="flex items-center justify-between border rounded-lg p-3 bg-card">
                <div className="flex items-center gap-3">
                  <button
                    onClick={() => toggleRule(rule.id, rule.enabled)}
                    className={`h-3 w-3 rounded-full ${rule.enabled ? "bg-green-500" : "bg-gray-400"}`}
                    title={rule.enabled ? "Enabled" : "Disabled"}
                  />
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-sm">{rule.event_type.replace(/_/g, " ")}</span>
                      <span className="text-xs text-muted-foreground px-1.5 py-0.5 bg-accent rounded">{dbName}</span>
                    </div>
                    <div className="text-xs text-muted-foreground mt-0.5">
                      {rule.webhook_url && <span>Webhook </span>}
                      {rule.email && <span>Email: {rule.email}</span>}
                    </div>
                  </div>
                </div>
                <Button variant="ghost" size="sm" onClick={() => deleteRule(rule.id)}>
                  <Trash2 className="h-4 w-4 text-destructive" />
                </Button>
              </div>
            );
          })}
        </div>
      )}

      {showHistory && (
        <div className="border rounded-lg p-4 bg-card space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="font-semibold">Alert History</h3>
            <Button variant="ghost" size="sm" onClick={() => setShowHistory(false)}>Close</Button>
          </div>
          {history.length === 0 ? (
            <p className="text-sm text-muted-foreground">No alerts sent yet</p>
          ) : (
            <div className="space-y-1 max-h-60 overflow-y-auto">
              {history.map((h) => (
                <div key={h.id} className="flex items-center justify-between text-sm py-1.5 border-b last:border-0">
                  <div>
                    <span className="font-medium">{h.event_type.replace(/_/g, " ")}</span>
                    <span className="text-muted-foreground ml-2">{h.message}</span>
                  </div>
                  <span className="text-xs text-muted-foreground whitespace-nowrap">
                    {new Date(h.sent_at).toLocaleString()}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
