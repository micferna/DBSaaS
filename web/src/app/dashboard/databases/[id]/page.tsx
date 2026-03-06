"use client";

import { useEffect, useState, useCallback, use } from "react";
import { useAuth } from "@/lib/auth";
import { api, API_URL, DatabaseInstance, DatabaseUser, DbEvent, MigrationRecord, BackupRecord, DatabaseStats, PrivateNetwork, NetworkPeering, BackupSchedule, PlanTemplate } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import {
  Play,
  Square,
  RotateCcw,
  KeyRound,
  Copy,
  UserPlus,
  Trash2,
  ShieldCheck,
  Database,
  ArrowLeft,
  Cpu,
  MemoryStick,
  Globe,
  Lock,
  Upload,
  HardDrive,
  Download,
  Loader2,
  Network,
  ArrowRightLeft,
  Pencil,
  Check,
  X,
  Clock,
  Layers,
  FileDown,
} from "lucide-react";
import Link from "next/link";
import {
  AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer,
} from "recharts";

const dbTypeLabel: Record<string, string> = {
  postgresql: "PostgreSQL",
  redis: "Redis",
  mariadb: "MariaDB",
};

const permissionConfig: Record<string, { color: string; label: string }> = {
  admin: { color: "bg-red-500/10 text-red-400 border-red-500/20", label: "Admin" },
  read_write: { color: "bg-blue-500/10 text-blue-400 border-blue-500/20", label: "Read/Write" },
  read_only: { color: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20", label: "Read Only" },
};

const statusConfig: Record<string, { color: string; dot: string }> = {
  running: { color: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20", dot: "bg-emerald-400" },
  provisioning: { color: "bg-amber-500/10 text-amber-400 border-amber-500/20", dot: "bg-amber-400 animate-pulse" },
  stopped: { color: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20", dot: "bg-zinc-400" },
  error: { color: "bg-red-500/10 text-red-400 border-red-500/20", dot: "bg-red-400" },
  deleting: { color: "bg-orange-500/10 text-orange-400 border-orange-500/20", dot: "bg-orange-400 animate-pulse" },
};

function formatBytes(bytes: number) {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

// Connection string format options per db type
type ConnFormat = "uri" | "psql" | "jdbc" | "python" | "nodejs" | "go" | "redis-cli";

function getConnFormats(dbType: string): { value: ConnFormat; label: string }[] {
  if (dbType === "postgresql") {
    return [
      { value: "uri", label: "URI" },
      { value: "psql", label: "psql" },
      { value: "jdbc", label: "JDBC" },
      { value: "python", label: "Python (psycopg2)" },
      { value: "nodejs", label: "Node.js (pg)" },
      { value: "go", label: "Go" },
    ];
  }
  if (dbType === "mariadb") {
    return [
      { value: "uri", label: "URI" },
      { value: "jdbc", label: "JDBC" },
      { value: "python", label: "Python (pymysql)" },
      { value: "nodejs", label: "Node.js (mysql2)" },
      { value: "go", label: "Go" },
    ];
  }
  if (dbType === "redis") {
    return [
      { value: "uri", label: "URI" },
      { value: "redis-cli", label: "redis-cli" },
      { value: "python", label: "Python (redis-py)" },
      { value: "nodejs", label: "Node.js (ioredis)" },
      { value: "go", label: "Go (go-redis)" },
    ];
  }
  return [{ value: "uri", label: "URI" }];
}

function formatConnectionString(db: DatabaseInstance, format: ConnFormat): string {
  const url = db.connection_url;
  if (format === "uri") return url;
  if (format === "psql") return `psql "${url}"`;
  if (format === "jdbc") {
    if (db.db_type === "postgresql") {
      const u = new URL(url);
      return `jdbc:postgresql://${u.host}${u.pathname}?sslmode=${db.ssl_mode || "require"}`;
    }
    if (db.db_type === "mariadb") {
      const u = new URL(url);
      return `jdbc:mariadb://${u.host}${u.pathname}?useSSL=true`;
    }
    return url;
  }
  if (format === "python") {
    if (db.db_type === "postgresql") return `import psycopg2\nconn = psycopg2.connect("${url}")`;
    if (db.db_type === "mariadb") return `import pymysql\nconn = pymysql.connect(host="${db.host}", port=${db.port}, user="${db.username}", password="...", database="${db.database_name || ""}")`;
    if (db.db_type === "redis") return `import redis\nr = redis.from_url("${url}")`;
    return url;
  }
  if (format === "nodejs") {
    if (db.db_type === "postgresql") return `const { Client } = require('pg');\nconst client = new Client({ connectionString: "${url}" });\nawait client.connect();`;
    if (db.db_type === "mariadb") return `const mysql = require('mysql2/promise');\nconst conn = await mysql.createConnection("${url}");`;
    if (db.db_type === "redis") return `const Redis = require('ioredis');\nconst redis = new Redis("${url}");`;
    return url;
  }
  if (format === "go") {
    if (db.db_type === "postgresql") return `import "database/sql"\ndb, err := sql.Open("postgres", "${url}")`;
    if (db.db_type === "mariadb") return `import "database/sql"\ndb, err := sql.Open("mysql", "${db.username}:...@tcp(${db.host}:${db.port})/${db.database_name || ""}")`;
    if (db.db_type === "redis") return `import "github.com/redis/go-redis/v9"\nrdb := redis.NewClient(&redis.Options{Addr: "${db.host}:${db.port}"})`;
    return url;
  }
  if (format === "redis-cli") return `redis-cli -u "${url}"`;
  return url;
}

export default function DatabaseDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params);
  const { token } = useAuth();
  const [db, setDb] = useState<DatabaseInstance | null>(null);
  const [migrations, setMigrations] = useState<MigrationRecord[]>([]);
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [dbUsers, setDbUsers] = useState<DatabaseUser[]>([]);
  const [newUsername, setNewUsername] = useState("");
  const [newPermission, setNewPermission] = useState<"admin" | "read_write" | "read_only">("read_only");
  const [createdPassword, setCreatedPassword] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [backupLoading, setBackupLoading] = useState(false);
  const [liveStats, setLiveStats] = useState<DatabaseStats | null>(null);
  const [statsHistory, setStatsHistory] = useState<Array<{ time: string; cpu: number; mem: number }>>([]);
  const [dbNetworks, setDbNetworks] = useState<PrivateNetwork[]>([]);
  const [dbPeerings, setDbPeerings] = useState<NetworkPeering[]>([]);

  // Backup schedule
  const [schedule, setSchedule] = useState<BackupSchedule | null>(null);
  const [scheduleLoaded, setScheduleLoaded] = useState(false);
  const [schedInterval, setSchedInterval] = useState(24);
  const [schedRetention, setSchedRetention] = useState(7);
  const [schedEnabled, setSchedEnabled] = useState(true);
  const [schedLoading, setSchedLoading] = useState(false);

  // Export
  const [exportLoading, setExportLoading] = useState(false);
  const [exportLink, setExportLink] = useState<{ filename: string; url: string } | null>(null);

  // Scale (change plan)
  const [plans, setPlans] = useState<PlanTemplate[]>([]);
  const [selectedPlanId, setSelectedPlanId] = useState("");
  const [scaleLoading, setScaleLoading] = useState(false);

  // Rename
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  const [renameLoading, setRenameLoading] = useState(false);

  // Clone
  const [cloneLoading, setCloneLoading] = useState<string | null>(null);

  // Connection string format
  const [connFormat, setConnFormat] = useState<ConnFormat>("uri");

  const fetchData = useCallback(async () => {
    if (!token) return;
    try {
      const [dbData, users, bkps] = await Promise.all([
        api.databases.get(token, id),
        api.databases.listUsers(token, id),
        api.databases.listBackups(token, id),
      ]);
      setDb(dbData);
      setDbUsers(users);
      setBackups(bkps);
    } catch {
      // silent on poll
    }
  }, [token, id]);

  // Initial fetch + polling every 5s + SSE for instant updates
  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 5000);
    return () => clearInterval(interval);
  }, [fetchData]);

  // One-time loads (migrations, networks, schedule, plans)
  useEffect(() => {
    if (!token) return;
    api.databases.listMigrations(token, id).then(setMigrations).catch(() => {});
    api.networks.list(token).then((nets) => {
      const myNets = nets.filter(n => n.members.some(m => m.database_id === id));
      setDbNetworks(myNets);
    }).catch(() => {});
    api.peerings.list(token).then((peers) => {
      setDbPeerings(peers);
    }).catch(() => {});
    api.databases.getBackupSchedule(token, id).then((s) => {
      setSchedule(s);
      if (s) {
        setSchedInterval(s.interval_hours);
        setSchedRetention(s.retention_count);
        setSchedEnabled(s.enabled);
      }
      setScheduleLoaded(true);
    }).catch(() => { setScheduleLoaded(true); });
    api.plans.list(token).then(setPlans).catch(() => {});
  }, [token, id]);

  useEffect(() => {
    if (!token) return;
    const es = new EventSource(`${API_URL}/api/databases/events?token=${encodeURIComponent(token)}`);

    es.addEventListener("status_changed", (e) => {
      const event: DbEvent = JSON.parse(e.data);
      if (event.database_id === id && event.status) {
        // Re-fetch full data to get connection_url, port, etc.
        fetchData();
      }
    });

    es.onerror = () => {};

    return () => es.close();
  }, [token, id]);

  // Live stats polling (every 3s)
  useEffect(() => {
    if (!token) return;
    const fetchStats = async () => {
      try {
        const s = await api.databases.stats(token, id);
        setLiveStats(s);
        const now = new Date().toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
        const memPct = s.memory_limit_bytes > 0 ? (s.memory_usage_bytes / s.memory_limit_bytes) * 100 : 0;
        setStatsHistory(prev => {
          const next = [...prev, { time: now, cpu: Math.round(s.cpu_percent * 10) / 10, mem: Math.round(memPct * 10) / 10 }];
          return next.slice(-30); // keep last 30 data points (~90s)
        });
      } catch {}
    };
    fetchStats();
    const interval = setInterval(fetchStats, 3000);
    return () => clearInterval(interval);
  }, [token, id]);

  const copy = (text: string, label = "Copied") => {
    navigator.clipboard.writeText(text);
    toast.success(label);
  };

  const handleContainerAction = async (action: "start" | "stop" | "restart") => {
    if (!token) return;
    setActionLoading(action);
    try {
      await api.databases.containerAction(token, id, action);
      toast.success(`Container ${action}ed`);
      fetchData();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : `Failed to ${action}`);
    } finally {
      setActionLoading(null);
    }
  };

  const handleRotateOwnerPassword = async () => {
    if (!token) return;
    setActionLoading("rotate");
    try {
      const res = await api.databases.rotateOwnerPassword(token, id);
      copy(res.password, "New password copied to clipboard");
      fetchData();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to rotate password");
    } finally {
      setActionLoading(null);
    }
  };

  const handleCreateUser = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token || !newUsername) return;
    try {
      const user = await api.databases.createUser(token, id, newUsername, newPermission);
      toast.success(`User "${user.username}" created`);
      setCreatedPassword(user.password || null);
      setNewUsername("");
      fetchData();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create user");
    }
  };

  const handleDeleteUser = async (userId: string, username: string) => {
    if (!token || !confirm(`Delete user "${username}"?`)) return;
    try {
      await api.databases.deleteUser(token, id, userId);
      toast.success("User deleted");
      fetchData();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete user");
    }
  };

  const handleRotateUserPassword = async (userId: string) => {
    if (!token) return;
    try {
      const res = await api.databases.rotateUserPassword(token, id, userId);
      copy(res.password, "New password copied to clipboard");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to rotate password");
    }
  };

  const handleCreateBackup = async () => {
    if (!token) return;
    setBackupLoading(true);
    try {
      await api.databases.createBackup(token, id);
      toast.success("Backup created successfully");
      fetchData();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create backup");
    } finally {
      setBackupLoading(false);
    }
  };

  const handleDeleteBackup = async (backupId: string, filename: string) => {
    if (!token || !confirm(`Delete backup "${filename}"?`)) return;
    try {
      await api.databases.deleteBackup(token, id, backupId);
      toast.success("Backup deleted");
      fetchData();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete backup");
    }
  };

  const handleClone = async (backupId: string) => {
    if (!token) return;
    const name = prompt("Enter a name for the cloned database:");
    if (!name?.trim()) return;
    setCloneLoading(backupId);
    try {
      const res = await api.databases.clone(token, id, backupId, name.trim());
      toast.success(`Clone "${res.name}" created — provisioning started`);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Clone failed");
    } finally {
      setCloneLoading(null);
    }
  };

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!token || !e.target.files?.[0]) return;
    try {
      const record = await api.databases.uploadMigration(token, id, e.target.files[0]);
      toast.success(`Migration "${record.filename}" applied`);
      setMigrations((prev) => [...prev, record]);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Migration failed");
    }
  };

  // Backup schedule handlers
  const handleSaveSchedule = async () => {
    if (!token) return;
    setSchedLoading(true);
    try {
      const opts = { interval_hours: schedInterval, retention_count: schedRetention, enabled: schedEnabled };
      let saved: BackupSchedule;
      if (schedule) {
        saved = await api.databases.updateBackupSchedule(token, id, opts);
        toast.success("Schedule updated");
      } else {
        saved = await api.databases.createBackupSchedule(token, id, opts);
        toast.success("Schedule created");
      }
      setSchedule(saved);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to save schedule");
    } finally {
      setSchedLoading(false);
    }
  };

  const handleDeleteSchedule = async () => {
    if (!token || !confirm("Delete backup schedule?")) return;
    setSchedLoading(true);
    try {
      await api.databases.deleteBackupSchedule(token, id);
      setSchedule(null);
      toast.success("Schedule deleted");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete schedule");
    } finally {
      setSchedLoading(false);
    }
  };

  // Export handler
  const handleExport = async () => {
    if (!token) return;
    setExportLoading(true);
    setExportLink(null);
    try {
      const res = await api.databases.exportDatabase(token, id);
      const url = api.databases.downloadExport(token, id, res.filename);
      setExportLink({ filename: res.filename, url });
      toast.success(`Export ready: ${res.filename} (${formatBytes(res.size_bytes)})`);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Export failed");
    } finally {
      setExportLoading(false);
    }
  };

  // Scale handler
  const handleScale = async () => {
    if (!token || !selectedPlanId) return;
    if (!confirm("Change plan? This will apply new resource limits.")) return;
    setScaleLoading(true);
    try {
      await api.databases.scale(token, id, selectedPlanId);
      toast.success("Plan changed successfully");
      fetchData();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to change plan");
    } finally {
      setScaleLoading(false);
    }
  };

  // Rename handler
  const handleRename = async () => {
    if (!token || !renameValue.trim()) return;
    setRenameLoading(true);
    try {
      await api.databases.rename(token, id, renameValue.trim());
      toast.success("Database renamed");
      setRenaming(false);
      fetchData();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to rename");
    } finally {
      setRenameLoading(false);
    }
  };

  if (!db) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  const sc = statusConfig[db.status] || statusConfig.error;
  const isRunning = db.status === "running";

  const samePlanPlans = plans.filter(p => p.db_type === db.db_type && p.active && !p.is_bundle);
  const connFormats = getConnFormats(db.db_type);
  const connString = formatConnectionString(db, connFormat);

  return (
    <div className="space-y-6 max-w-3xl">
      {/* Back + Header */}
      <div>
        <Link href="/dashboard" className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors mb-4">
          <ArrowLeft className="h-3 w-3" /> Back to databases
        </Link>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="h-10 w-10 rounded-lg bg-gradient-to-br from-blue-500/20 to-violet-500/20 border border-border/50 flex items-center justify-center">
              <Database className="h-5 w-5 text-blue-400" />
            </div>
            <div>
              {/* Inline rename */}
              {renaming ? (
                <div className="flex items-center gap-2">
                  <Input
                    value={renameValue}
                    onChange={(e) => setRenameValue(e.target.value)}
                    className="h-7 text-sm w-48"
                    autoFocus
                    onKeyDown={(e) => { if (e.key === "Enter") handleRename(); if (e.key === "Escape") setRenaming(false); }}
                  />
                  <Button size="sm" variant="ghost" className="h-7 w-7 p-0" onClick={handleRename} disabled={renameLoading}>
                    {renameLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Check className="h-3.5 w-3.5 text-emerald-400" />}
                  </Button>
                  <Button size="sm" variant="ghost" className="h-7 w-7 p-0" onClick={() => setRenaming(false)}>
                    <X className="h-3.5 w-3.5 text-muted-foreground" />
                  </Button>
                </div>
              ) : (
                <div className="flex items-center gap-1.5">
                  <h1 className="text-xl font-bold">{db.name}</h1>
                  <button
                    className="text-muted-foreground hover:text-foreground transition-colors"
                    onClick={() => { setRenameValue(db.name); setRenaming(true); }}
                    title="Rename"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                </div>
              )}
              <div className="flex items-center gap-2 mt-0.5">
                <Badge variant="outline" className="text-[10px] font-normal gap-1">
                  <Database className="h-2.5 w-2.5" />
                  {dbTypeLabel[db.db_type] || db.db_type}
                </Badge>
                <Badge variant="outline" className={`text-[10px] font-normal gap-1 ${sc.color} border`}>
                  <span className={`h-1.5 w-1.5 rounded-full ${sc.dot}`} />
                  {db.status}
                </Badge>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Actions */}
      <div className="flex gap-2 flex-wrap">
        <Button
          size="sm"
          variant="outline"
          className="h-8 gap-1.5 text-xs"
          onClick={() => handleContainerAction("start")}
          disabled={!!actionLoading || isRunning}
        >
          <Play className="h-3 w-3" /> Start
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="h-8 gap-1.5 text-xs"
          onClick={() => handleContainerAction("stop")}
          disabled={!!actionLoading || db.status === "stopped"}
        >
          <Square className="h-3 w-3" /> Stop
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="h-8 gap-1.5 text-xs"
          onClick={() => handleContainerAction("restart")}
          disabled={!!actionLoading}
        >
          <RotateCcw className={`h-3 w-3 ${actionLoading === "restart" ? "animate-spin" : ""}`} /> Restart
        </Button>
        <div className="w-px h-8 bg-border/50" />
        <Button
          size="sm"
          variant="outline"
          className="h-8 gap-1.5 text-xs"
          onClick={handleRotateOwnerPassword}
          disabled={!isRunning || !!actionLoading}
        >
          <KeyRound className="h-3 w-3" /> Rotate Password
        </Button>
      </div>

      {/* Connection Details */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Globe className="h-4 w-4 text-muted-foreground" /> Connection
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-3 text-sm">
            {[
              { label: "Host", value: db.host },
              { label: "Port", value: db.port },
              { label: "Username", value: db.username },
              { label: "Password", value: db.password, mono: true },
              ...(db.database_name ? [{ label: "Database", value: db.database_name }] : []),
              { label: "Security", value: db.ssl_mode === "verify-ca" ? "Verified TLS (CA cert)" : "Standard TLS", icon: true },
            ].map((item) => (
              <div key={item.label} className="space-y-0.5">
                <p className="text-[11px] text-muted-foreground uppercase tracking-wider">{item.label}</p>
                <p className={`text-sm ${item.mono ? "font-mono text-xs" : ""} flex items-center gap-1`}>
                  {item.icon && <Lock className="h-3 w-3 text-emerald-400" />}
                  {String(item.value)}
                </p>
              </div>
            ))}
          </div>

          {/* Connection string format selector */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <p className="text-[11px] text-muted-foreground uppercase tracking-wider">Connection String</p>
              <select
                value={connFormat}
                onChange={(e) => setConnFormat(e.target.value as ConnFormat)}
                className="h-6 rounded border border-input bg-background px-1.5 text-[11px]"
              >
                {connFormats.map(f => (
                  <option key={f.value} value={f.value}>{f.label}</option>
                ))}
              </select>
            </div>
            <div className="flex items-start gap-2">
              <code className="flex-1 text-[11px] font-mono bg-muted/50 px-3 py-2 rounded-md break-all text-muted-foreground whitespace-pre-wrap">
                {connString}
              </code>
              <Button size="sm" variant="ghost" className="h-8 w-8 p-0 shrink-0 mt-0" onClick={() => copy(connString, "Copied")}>
                <Copy className="h-3.5 w-3.5" />
              </Button>
            </div>
          </div>

          {db.ssl_mode === "verify-ca" && (
            <div className="mt-3 p-3 rounded-lg border border-blue-500/20 bg-blue-500/5">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-xs font-medium text-blue-400">CA Certificate required</p>
                  <p className="text-[11px] text-muted-foreground mt-0.5">Download and configure the CA certificate for TLS verification.</p>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  className="text-xs h-7 border-blue-500/30 text-blue-400 hover:bg-blue-500/10"
                  onClick={async () => {
                    if (!token) return;
                    try {
                      const cert = await api.databases.getCaCert(token);
                      const blob = new Blob([cert], { type: "application/x-pem-file" });
                      const url = URL.createObjectURL(blob);
                      const a = document.createElement("a");
                      a.href = url;
                      a.download = "dbsaas-ca.crt";
                      a.click();
                      URL.revokeObjectURL(url);
                      toast.success("CA certificate downloaded");
                    } catch {
                      toast.error("Failed to download certificate");
                    }
                  }}
                >
                  <Download className="h-3 w-3 mr-1" />
                  Download CA
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Networks & Peering */}
      {dbNetworks.length > 0 && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2">
              <Network className="h-4 w-4 text-muted-foreground" /> Networks & Peering
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {/* Networks this DB belongs to */}
            <div className="space-y-2">
              <p className="text-[11px] text-muted-foreground uppercase tracking-wider">Member of</p>
              <div className="space-y-1.5">
                {dbNetworks.map((net) => {
                  const networkPeerings = dbPeerings.filter(
                    p => p.network_a.id === net.id || p.network_b.id === net.id
                  );
                  return (
                    <div key={net.id} className="rounded-lg border px-3 py-2 space-y-2">
                      <div className="flex items-center gap-3">
                        <Network className="h-3.5 w-3.5 text-muted-foreground" />
                        <span className="text-sm font-medium">{net.name}</span>
                        <Badge variant="secondary" className="text-xs">
                          {net.members.length} member{net.members.length !== 1 ? "s" : ""}
                        </Badge>
                        {net.subnet && (
                          <code className="text-[11px] bg-muted px-1.5 py-0.5 rounded font-mono text-muted-foreground">
                            {net.subnet}
                          </code>
                        )}
                      </div>

                      {/* Other members in same network */}
                      {net.members.filter(m => m.database_id !== id).length > 0 && (
                        <div className="ml-7 space-y-1">
                          <p className="text-[10px] text-muted-foreground">Reachable databases:</p>
                          {net.members.filter(m => m.database_id !== id).map((member) => (
                            <div key={member.database_id} className="flex items-center gap-2 text-xs text-muted-foreground">
                              <Database className="h-3 w-3" />
                              <span>{member.database_name}</span>
                              <Badge variant="secondary" className={`text-[10px] ${
                                member.db_type === "postgresql" ? "bg-blue-500/10 text-blue-600" :
                                member.db_type === "redis" ? "bg-red-500/10 text-red-600" :
                                "bg-emerald-500/10 text-emerald-600"
                              }`}>
                                {member.db_type}
                              </Badge>
                              <code className="bg-muted px-1 py-0.5 rounded font-mono">
                                {member.hostname}:{member.port}
                              </code>
                            </div>
                          ))}
                        </div>
                      )}

                      {/* Peerings for this network */}
                      {networkPeerings.length > 0 && (
                        <div className="ml-7 space-y-1">
                          <p className="text-[10px] text-muted-foreground flex items-center gap-1">
                            <ArrowRightLeft className="h-3 w-3" /> Peered with:
                          </p>
                          {networkPeerings.map((p) => {
                            const otherNet = p.network_a.id === net.id ? p.network_b : p.network_a;
                            return (
                              <div key={p.id} className="flex items-center gap-2 text-xs text-muted-foreground">
                                <Network className="h-3 w-3" />
                                <span>{otherNet.name}</span>
                                <Badge variant="secondary" className={`text-[10px] ${
                                  p.status === "active" ? "bg-green-500/10 text-green-600" :
                                  p.status === "pending" ? "bg-yellow-500/10 text-yellow-600" :
                                  "bg-red-500/10 text-red-600"
                                }`}>
                                  {p.status}
                                </Badge>
                                {p.rules.length > 0 && (
                                  <span className="text-[10px]">
                                    ({p.rules.filter(r => r.action === "allow").length} allow rule{p.rules.filter(r => r.action === "allow").length !== 1 ? "s" : ""})
                                  </span>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            {/* Internal hostname info */}
            <div className="rounded-lg bg-muted/50 p-3">
              <p className="text-xs text-muted-foreground">
                <Network className="h-3 w-3 inline mr-1" />
                This database is reachable on the network{dbNetworks.length > 1 ? "s" : ""} as{" "}
                <code className="bg-muted px-1 rounded font-mono">
                  sb-{id}:{db.port}
                </code>
              </p>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Resources — Live Stats */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Cpu className="h-4 w-4 text-muted-foreground" /> Resources
            {liveStats && (
              <span className="ml-auto flex items-center gap-1.5 text-[10px] font-normal text-emerald-400">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" /> Live
              </span>
            )}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Gauges */}
          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 rounded-lg bg-muted/30 border border-border/30 space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Cpu className="h-3.5 w-3.5 text-blue-400" />
                  <span className="text-xs text-muted-foreground">CPU</span>
                </div>
                <span className="text-sm font-mono font-medium">
                  {liveStats ? `${liveStats.cpu_percent.toFixed(1)}%` : "—"}
                </span>
              </div>
              <div className="h-2 rounded-full bg-muted/50 overflow-hidden">
                <div
                  className="h-full rounded-full bg-blue-500 transition-all duration-500"
                  style={{ width: `${Math.min(liveStats?.cpu_percent ?? 0, 100)}%` }}
                />
              </div>
              <p className="text-[10px] text-muted-foreground">{db.cpu_limit} cores allocated</p>
            </div>
            <div className="p-3 rounded-lg bg-muted/30 border border-border/30 space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <MemoryStick className="h-3.5 w-3.5 text-violet-400" />
                  <span className="text-xs text-muted-foreground">Memory</span>
                </div>
                <span className="text-sm font-mono font-medium">
                  {liveStats ? formatBytes(liveStats.memory_usage_bytes) : "—"}
                </span>
              </div>
              <div className="h-2 rounded-full bg-muted/50 overflow-hidden">
                <div
                  className="h-full rounded-full bg-violet-500 transition-all duration-500"
                  style={{
                    width: `${liveStats && liveStats.memory_limit_bytes > 0
                      ? Math.min((liveStats.memory_usage_bytes / liveStats.memory_limit_bytes) * 100, 100)
                      : 0}%`,
                  }}
                />
              </div>
              <p className="text-[10px] text-muted-foreground">{db.memory_limit_mb} MB allocated</p>
            </div>
          </div>

          {/* Disk usage */}
          {liveStats?.disk_usage_bytes !== undefined && (
            <div className="p-3 rounded-lg bg-muted/30 border border-border/30 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <HardDrive className="h-3.5 w-3.5 text-amber-400" />
                <span className="text-xs text-muted-foreground">Disk Usage</span>
              </div>
              <span className="text-sm font-mono font-medium">{formatBytes(liveStats.disk_usage_bytes)}</span>
            </div>
          )}

          {/* History Chart */}
          {statsHistory.length > 1 && (
            <div className="pt-2">
              <p className="text-[11px] text-muted-foreground uppercase tracking-wider mb-2">Usage History</p>
              <div className="h-40 w-full">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={statsHistory} margin={{ top: 4, right: 4, bottom: 0, left: -20 }}>
                    <defs>
                      <linearGradient id="cpuGrad" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="#3b82f6" stopOpacity={0.3} />
                        <stop offset="100%" stopColor="#3b82f6" stopOpacity={0} />
                      </linearGradient>
                      <linearGradient id="memGrad" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="#8b5cf6" stopOpacity={0.3} />
                        <stop offset="100%" stopColor="#8b5cf6" stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <XAxis dataKey="time" tick={{ fontSize: 10, fill: "#888" }} tickLine={false} axisLine={false} />
                    <YAxis tick={{ fontSize: 10, fill: "#888" }} tickLine={false} axisLine={false} domain={[0, "auto"]} unit="%" />
                    <Tooltip
                      contentStyle={{ background: "#1a1a2e", border: "1px solid #333", borderRadius: 8, fontSize: 12 }}
                      labelStyle={{ color: "#aaa" }}
                    />
                    <Area type="monotone" dataKey="cpu" name="CPU" stroke="#3b82f6" fill="url(#cpuGrad)" strokeWidth={1.5} dot={false} />
                    <Area type="monotone" dataKey="mem" name="Memory" stroke="#8b5cf6" fill="url(#memGrad)" strokeWidth={1.5} dot={false} />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Backups */}
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm flex items-center gap-2">
              <HardDrive className="h-4 w-4 text-muted-foreground" /> Backups
            </CardTitle>
            <div className="flex items-center gap-2">
              {/* Export button */}
              <Button
                size="sm"
                variant="outline"
                className="h-7 gap-1.5 text-xs"
                onClick={handleExport}
                disabled={!isRunning || exportLoading}
              >
                {exportLoading ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <FileDown className="h-3 w-3" />
                )}
                {exportLoading ? "Exporting..." : "Export"}
              </Button>
              <Button
                size="sm"
                variant="outline"
                className="h-7 gap-1.5 text-xs"
                onClick={handleCreateBackup}
                disabled={!isRunning || backupLoading}
              >
                {backupLoading ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <Download className="h-3 w-3" />
                )}
                {backupLoading ? "Creating..." : "Create Backup"}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          {/* Export download link */}
          {exportLink && (
            <div className="flex items-center gap-3 p-2.5 rounded-lg border border-emerald-500/20 bg-emerald-500/5">
              <FileDown className="h-3.5 w-3.5 text-emerald-400 shrink-0" />
              <span className="text-xs text-muted-foreground flex-1 font-mono truncate">{exportLink.filename}</span>
              <a
                href={exportLink.url}
                download={exportLink.filename}
                className="text-xs text-emerald-400 hover:text-emerald-300 underline shrink-0"
              >
                Download
              </a>
              <button onClick={() => setExportLink(null)} className="text-muted-foreground hover:text-foreground">
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          )}

          {backups.length > 0 ? (
            <div className="space-y-1">
              {backups.map((b) => (
                <div key={b.id} className="flex items-center justify-between py-2 px-3 rounded-lg hover:bg-muted/30 transition-colors group">
                  <div className="flex items-center gap-3">
                    <HardDrive className="h-3.5 w-3.5 text-muted-foreground" />
                    <div>
                      <p className="text-xs font-mono">{b.filename}</p>
                      <p className="text-[10px] text-muted-foreground">
                        {formatBytes(b.size_bytes)} &middot; {new Date(b.created_at).toLocaleString()}
                      </p>
                    </div>
                  </div>
                  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 text-xs gap-1"
                      onClick={() => handleClone(b.id)}
                      disabled={cloneLoading === b.id}
                    >
                      {cloneLoading === b.id ? (
                        <Loader2 className="h-3 w-3 animate-spin" />
                      ) : (
                        <Layers className="h-3 w-3" />
                      )}
                      Clone
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 text-xs gap-1 text-destructive hover:text-destructive"
                      onClick={() => handleDeleteBackup(b.id, b.filename)}
                    >
                      <Trash2 className="h-3 w-3" /> Delete
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-xs text-muted-foreground/60 text-center py-4">No backups yet</p>
          )}
        </CardContent>
      </Card>

      {/* Backup Schedule */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Clock className="h-4 w-4 text-muted-foreground" /> Backup Schedule
            {schedule && (
              <Badge variant="outline" className={`ml-auto text-[10px] font-normal ${schedule.enabled ? "bg-emerald-500/10 text-emerald-400 border-emerald-500/20" : "bg-zinc-500/10 text-zinc-400 border-zinc-500/20"}`}>
                {schedule.enabled ? "Enabled" : "Disabled"}
              </Badge>
            )}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {scheduleLoaded && schedule && (
            <div className="rounded-lg bg-muted/30 border border-border/30 px-3 py-2 text-xs text-muted-foreground space-y-0.5">
              <p>Every <span className="text-foreground font-medium">{schedule.interval_hours}h</span> &middot; Keep last <span className="text-foreground font-medium">{schedule.retention_count}</span> backups</p>
              {schedule.last_run_at && (
                <p>Last run: {new Date(schedule.last_run_at).toLocaleString()}</p>
              )}
            </div>
          )}

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <Label className="text-xs text-muted-foreground">Interval (hours, 1–168)</Label>
              <Input
                type="number"
                min={1}
                max={168}
                value={schedInterval}
                onChange={(e) => setSchedInterval(Number(e.target.value))}
                className="h-8 text-sm"
              />
            </div>
            <div className="space-y-1">
              <Label className="text-xs text-muted-foreground">Retention count (1–30)</Label>
              <Input
                type="number"
                min={1}
                max={30}
                value={schedRetention}
                onChange={(e) => setSchedRetention(Number(e.target.value))}
                className="h-8 text-sm"
              />
            </div>
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="sched-enabled"
              checked={schedEnabled}
              onChange={(e) => setSchedEnabled(e.target.checked)}
              className="h-4 w-4 rounded border-input"
            />
            <Label htmlFor="sched-enabled" className="text-xs cursor-pointer">Enable auto-backups</Label>
          </div>

          <div className="flex items-center gap-2">
            <Button
              size="sm"
              className="h-8 gap-1.5 text-xs"
              onClick={handleSaveSchedule}
              disabled={schedLoading}
            >
              {schedLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : <Check className="h-3 w-3" />}
              {schedule ? "Update Schedule" : "Create Schedule"}
            </Button>
            {schedule && (
              <Button
                size="sm"
                variant="outline"
                className="h-8 gap-1.5 text-xs text-destructive hover:text-destructive border-destructive/30"
                onClick={handleDeleteSchedule}
                disabled={schedLoading}
              >
                <Trash2 className="h-3 w-3" /> Delete Schedule
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Change Plan (Scale) */}
      {samePlanPlans.length > 0 && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2">
              <Layers className="h-4 w-4 text-muted-foreground" /> Change Plan
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <p className="text-xs text-muted-foreground">Select a new plan to scale this database. Current plan resources: {db.cpu_limit} CPU / {db.memory_limit_mb} MB RAM.</p>
            <div className="flex items-center gap-2">
              <select
                value={selectedPlanId}
                onChange={(e) => setSelectedPlanId(e.target.value)}
                className="flex-1 h-8 rounded-md border border-input bg-background px-2 text-sm"
              >
                <option value="">Select a plan...</option>
                {samePlanPlans.map(p => (
                  <option key={p.id} value={p.id}>
                    {p.name} — {p.cpu_limit} CPU / {p.memory_limit_mb} MB / {(p.monthly_price_cents / 100).toFixed(0)}€/mo
                  </option>
                ))}
              </select>
              <Button
                size="sm"
                className="h-8 gap-1.5 text-xs shrink-0"
                onClick={handleScale}
                disabled={!selectedPlanId || scaleLoading}
              >
                {scaleLoading ? <Loader2 className="h-3 w-3 animate-spin" /> : null}
                Apply
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Database Users */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <ShieldCheck className="h-4 w-4 text-muted-foreground" /> Database Users
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <form onSubmit={handleCreateUser} className="flex gap-2 items-end">
            <div className="flex-1">
              <Label htmlFor="new-username" className="text-xs text-muted-foreground">Username</Label>
              <Input
                id="new-username"
                value={newUsername}
                onChange={(e) => setNewUsername(e.target.value)}
                placeholder="myuser"
                pattern="^[a-zA-Z0-9_]+$"
                className="mt-1 h-8 text-sm"
                required
              />
            </div>
            <div>
              <Label htmlFor="permission" className="text-xs text-muted-foreground">Permission</Label>
              <select
                id="permission"
                value={newPermission}
                onChange={(e) => setNewPermission(e.target.value as "admin" | "read_write" | "read_only")}
                className="mt-1 h-8 rounded-md border border-input bg-background px-2 text-sm w-full"
              >
                <option value="read_only">Read Only</option>
                <option value="read_write">Read Write</option>
                <option value="admin">Admin</option>
              </select>
            </div>
            <Button type="submit" size="sm" className="h-8 gap-1 text-xs" disabled={!isRunning}>
              <UserPlus className="h-3 w-3" /> Add
            </Button>
          </form>

          {createdPassword && (
            <div className="bg-emerald-500/5 border border-emerald-500/20 rounded-lg p-3">
              <p className="text-xs font-medium text-emerald-400 mb-2">User created! Copy the password now — it won&apos;t be shown again.</p>
              <div className="flex items-center gap-2">
                <code className="flex-1 text-xs font-mono bg-background px-2.5 py-1.5 rounded break-all">{createdPassword}</code>
                <Button size="sm" variant="outline" className="h-7 text-xs shrink-0" onClick={() => { copy(createdPassword, "Password copied"); setCreatedPassword(null); }}>
                  <Copy className="h-3 w-3 mr-1" /> Copy
                </Button>
              </div>
            </div>
          )}

          {dbUsers.length > 0 ? (
            <div className="space-y-1">
              {dbUsers.map((u) => {
                const pc = permissionConfig[u.permission] || permissionConfig.read_only;
                return (
                  <div key={u.id} className="flex items-center justify-between py-2 px-3 rounded-lg hover:bg-muted/30 transition-colors group">
                    <div className="flex items-center gap-2.5">
                      <code className="text-sm font-mono">{u.username}</code>
                      <Badge variant="outline" className={`text-[10px] font-normal ${pc.color} border`}>
                        {pc.label}
                      </Badge>
                    </div>
                    <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                      <Button size="sm" variant="ghost" className="h-7 text-xs gap-1" onClick={() => handleRotateUserPassword(u.id)}>
                        <KeyRound className="h-3 w-3" /> Rotate
                      </Button>
                      <Button size="sm" variant="ghost" className="h-7 text-xs gap-1 text-destructive hover:text-destructive" onClick={() => handleDeleteUser(u.id, u.username)}>
                        <Trash2 className="h-3 w-3" /> Delete
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <p className="text-xs text-muted-foreground/60 text-center py-4">No additional users</p>
          )}
        </CardContent>
      </Card>

      {/* Migrations (PG and MariaDB) */}
      {(db.db_type === "postgresql" || db.db_type === "mariadb") && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2">
              <Upload className="h-4 w-4 text-muted-foreground" /> Migrations
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Input id="migration" type="file" accept=".sql" onChange={handleUpload} className="h-8 text-sm" />
            {migrations.length > 0 && (
              <div className="space-y-1">
                {migrations.map((m) => (
                  <div key={m.id} className="text-xs flex justify-between px-3 py-2 rounded bg-muted/30">
                    <span className="font-mono">{m.filename}</span>
                    <span className="text-muted-foreground">{new Date(m.applied_at).toLocaleDateString()}</span>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
