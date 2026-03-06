export const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:3001";
const API_BASE = API_URL;

interface RequestOptions {
  method?: string;
  body?: unknown;
  token?: string;
  headers?: Record<string, string>;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const { method = "GET", body, token, headers = {} } = options;

  const fetchHeaders: Record<string, string> = {
    ...headers,
  };

  if (token) {
    fetchHeaders["Authorization"] = `Bearer ${token}`;
  }

  if (body && !(body instanceof FormData)) {
    fetchHeaders["Content-Type"] = "application/json";
  }

  const res = await fetch(`${API_BASE}${path}`, {
    method,
    headers: fetchHeaders,
    body: body instanceof FormData ? body : body ? JSON.stringify(body) : undefined,
  });

  if (!res.ok) {
    const error = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(error.error || "Request failed");
  }

  const text = await res.text();
  return text ? JSON.parse(text) : ({} as T);
}

export interface DatabaseInstance {
  id: string;
  name: string;
  db_type: "postgresql" | "redis" | "mariadb";
  status: "provisioning" | "running" | "stopped" | "error" | "deleting";
  host: string;
  port: number;
  username: string;
  password: string;
  database_name: string | null;
  connection_url: string;
  tls_enabled: boolean;
  ssl_mode: string;
  cpu_limit: number;
  memory_limit_mb: number;
  bundle_id: string | null;
  plan_template_id: string | null;
  subdomain?: string;
  routing_mode?: string;
  created_at: string;
}

export interface DatabaseUser {
  id: string;
  database_id: string;
  username: string;
  password?: string;
  permission: "admin" | "read_write" | "read_only";
  created_at: string;
}

export interface BundleResponse {
  id: string;
  name: string;
  postgresql: DatabaseInstance;
  redis: DatabaseInstance;
  created_at: string;
}

export interface BackupRecord {
  id: string;
  database_id: string;
  filename: string;
  size_bytes: number;
  created_at: string;
}

export interface AuthResponse {
  token: string;
  user: { id: string; email: string; role: string };
}

export interface MigrationRecord {
  id: string;
  database_id: string;
  filename: string;
  checksum: string;
  applied_at: string;
}

export interface PlanTemplate {
  id: string;
  name: string;
  db_type: "postgresql" | "redis" | "mariadb";
  cpu_limit: number;
  memory_limit_mb: number;
  monthly_price_cents: number;
  hourly_price_cents: number;
  is_bundle: boolean;
  active: boolean;
  created_at: string;
}

export interface PrivateNetworkMemberInfo {
  database_id: string;
  database_name: string;
  db_type: "postgresql" | "redis" | "mariadb";
  hostname: string;
  port: number;
  joined_at: string;
}

export interface PrivateNetwork {
  id: string;
  name: string;
  docker_server_id: string | null;
  subnet: string | null;
  gateway: string | null;
  members: PrivateNetworkMemberInfo[];
  created_at: string;
}

export interface NetworkPeering {
  id: string;
  network_a: { id: string; name: string; member_count: number };
  network_b: { id: string; name: string; member_count: number };
  status: "pending" | "active" | "error";
  rules: FirewallRule[];
  created_at: string;
}

export interface FirewallRule {
  id: string;
  peering_id: string;
  priority: number;
  action: "allow" | "deny";
  source_network_id: string;
  dest_network_id: string;
  port: number | null;
  protocol: string | null;
  description: string | null;
  created_at: string;
}

export interface AvailableServer {
  id: string;
  name: string;
  region: string | null;
}

export interface DatabaseStats {
  status: string;
  cpu_percent: number;
  memory_usage_bytes: number;
  memory_limit_bytes: number;
  cpu_limit: number;
  memory_limit_mb: number;
  disk_usage_bytes?: number;
}

export interface BackupSchedule {
  id: string;
  database_id: string;
  interval_hours: number;
  retention_count: number;
  enabled: boolean;
  last_run_at: string | null;
  created_at: string;
}

export interface AuditLog {
  id: string;
  user_id: string | null;
  action: string;
  resource_type: string;
  resource_id: string | null;
  details: Record<string, unknown> | null;
  ip_address: string | null;
  created_at: string;
}

export interface AlertRule {
  id: string;
  user_id: string;
  database_id: string | null;
  event_type: string;
  webhook_url: string | null;
  email: string | null;
  enabled: boolean;
  created_at: string;
}

export interface AlertHistory {
  id: string;
  alert_rule_id: string;
  event_type: string;
  message: string;
  sent_at: string;
}

export interface BillingPeriod {
  id: string;
  user_id: string;
  period_start: string;
  period_end: string;
  total_cents: number;
  stripe_invoice_id: string | null;
  status: "pending" | "invoiced" | "paid" | "failed";
  created_at: string;
}

export interface BillingLineItem {
  id: string;
  billing_period_id: string;
  database_id: string;
  plan_template_id: string | null;
  hours_used: number;
  amount_cents: number;
  created_at: string;
}

export interface DbEvent {
  user_id: string;
  database_id: string;
  event_type: "status_changed" | "deleted";
  status: DatabaseInstance["status"] | null;
}

export interface CurrentUsage {
  databases: Array<{
    database_id: string;
    database_name: string;
    plan_name: string | null;
    hours_used: number;
    estimated_cents: number;
  }>;
  total_estimated_cents: number;
}

export const api = {
  auth: {
    register: (email: string, password: string, invitation_code?: string) =>
      request<AuthResponse>("/api/auth/register", {
        method: "POST",
        body: { email, password, invitation_code },
      }),
    login: (email: string, password: string) =>
      request<AuthResponse>("/api/auth/login", {
        method: "POST",
        body: { email, password },
      }),
    me: (token: string) => request<{ id: string; email: string; role: string }>("/api/auth/me", { token }),
    generateApiKey: (token: string) =>
      request<{ api_key: string }>("/api/auth/api-key", { method: "POST", token }),
  },
  databases: {
    list: (token: string) => request<DatabaseInstance[]>("/api/databases", { token }),
    listServers: (token: string) => request<AvailableServer[]>("/api/servers", { token }),
    create: (token: string, name: string, db_type: string, options?: { plan_template_id?: string; cpu_limit?: number; memory_limit_mb?: number; ssl_mode?: string; server_id?: string }) =>
      request<DatabaseInstance>("/api/databases", {
        method: "POST",
        token,
        body: { name, db_type, ...options },
      }),
    createBundle: (token: string, name: string, options?: { plan_template_id?: string; cpu_limit?: number; memory_limit_mb?: number; ssl_mode?: string; server_id?: string }) =>
      request<BundleResponse>("/api/databases/bundle", {
        method: "POST",
        token,
        body: { name, ...options },
      }),
    get: (token: string, id: string) => request<DatabaseInstance>(`/api/databases/${id}`, { token }),
    stats: (token: string, id: string) => request<DatabaseStats>(`/api/databases/${id}/stats`, { token }),
    delete: (token: string, id: string) =>
      request<{ status: string }>(`/api/databases/${id}`, { method: "DELETE", token }),
    containerAction: (token: string, id: string, action: "start" | "stop" | "restart") =>
      request<{ status: string }>(`/api/databases/${id}/action`, {
        method: "POST",
        token,
        body: { action },
      }),
    listUsers: (token: string, dbId: string) =>
      request<DatabaseUser[]>(`/api/databases/${dbId}/users`, { token }),
    createUser: (token: string, dbId: string, username: string, permission: string) =>
      request<DatabaseUser>(`/api/databases/${dbId}/users`, {
        method: "POST",
        token,
        body: { username, permission },
      }),
    deleteUser: (token: string, dbId: string, userId: string) =>
      request<{ status: string }>(`/api/databases/${dbId}/users/${userId}`, {
        method: "DELETE",
        token,
      }),
    rotateUserPassword: (token: string, dbId: string, userId: string) =>
      request<{ password: string }>(`/api/databases/${dbId}/users/${userId}/rotate-password`, {
        method: "POST",
        token,
      }),
    rotateOwnerPassword: (token: string, dbId: string) =>
      request<{ password: string }>(`/api/databases/${dbId}/rotate-password`, {
        method: "POST",
        token,
      }),
    getCaCert: (token: string) => request<string>("/api/databases/ca-cert", { token }),
    // Backups
    listBackups: (token: string, dbId: string) =>
      request<BackupRecord[]>(`/api/databases/${dbId}/backups`, { token }),
    createBackup: (token: string, dbId: string) =>
      request<BackupRecord>(`/api/databases/${dbId}/backups`, { method: "POST", token }),
    deleteBackup: (token: string, dbId: string, backupId: string) =>
      request<{ status: string }>(`/api/databases/${dbId}/backups/${backupId}`, {
        method: "DELETE",
        token,
      }),
    // Migrations
    uploadMigration: (token: string, dbId: string, file: File) => {
      const formData = new FormData();
      formData.append("file", file);
      return request<MigrationRecord>(`/api/databases/${dbId}/migrations`, {
        method: "POST",
        token,
        body: formData,
      });
    },
    listMigrations: (token: string, dbId: string) =>
      request<MigrationRecord[]>(`/api/databases/${dbId}/migrations`, { token }),
    // Backup schedule
    getBackupSchedule: (token: string, dbId: string) =>
      request<BackupSchedule | null>(`/api/databases/${dbId}/backup-schedule`, { token }),
    createBackupSchedule: (token: string, dbId: string, opts?: { interval_hours?: number; retention_count?: number; enabled?: boolean }) =>
      request<BackupSchedule>(`/api/databases/${dbId}/backup-schedule`, { method: "POST", token, body: opts || {} }),
    updateBackupSchedule: (token: string, dbId: string, opts: { interval_hours?: number; retention_count?: number; enabled?: boolean }) =>
      request<BackupSchedule>(`/api/databases/${dbId}/backup-schedule`, { method: "PUT", token, body: opts }),
    deleteBackupSchedule: (token: string, dbId: string) =>
      request<{ status: string }>(`/api/databases/${dbId}/backup-schedule`, { method: "DELETE", token }),
    // Export
    exportDatabase: (token: string, dbId: string) =>
      request<{ filename: string; size_bytes: number }>(`/api/databases/${dbId}/export`, { method: "POST", token }),
    downloadExport: (token: string, dbId: string, filename: string) =>
      `${API_BASE}/api/databases/${dbId}/export/${filename}`,
    // Scale / Rename / Clone
    scale: (token: string, dbId: string, plan_template_id: string) =>
      request<{ status: string }>(`/api/databases/${dbId}/scale`, { method: "PUT", token, body: { plan_template_id } }),
    rename: (token: string, dbId: string, name: string) =>
      request<{ status: string }>(`/api/databases/${dbId}/rename`, { method: "PUT", token, body: { name } }),
    clone: (token: string, dbId: string, backupId: string, name: string) =>
      request<{ id: string; name: string; status: string }>(`/api/databases/${dbId}/clone/${backupId}`, { method: "POST", token, body: { name } }),
    // Favorites
    addFavorite: (token: string, dbId: string) =>
      request<{ status: string }>(`/api/databases/${dbId}/favorite`, { method: "POST", token }),
    removeFavorite: (token: string, dbId: string) =>
      request<{ status: string }>(`/api/databases/${dbId}/favorite`, { method: "DELETE", token }),
    listFavorites: (token: string) =>
      request<string[]>("/api/databases/favorites", { token }),
  },
  networks: {
    list: (token: string) => request<PrivateNetwork[]>("/api/networks", { token }),
    get: (token: string, id: string) => request<PrivateNetwork>(`/api/networks/${id}`, { token }),
    create: (token: string, name: string) =>
      request<PrivateNetwork>("/api/networks", { method: "POST", token, body: { name } }),
    delete: (token: string, id: string) =>
      request<{ status: string }>(`/api/networks/${id}`, { method: "DELETE", token }),
    attach: (token: string, networkId: string, database_id: string) =>
      request<PrivateNetwork>(`/api/networks/${networkId}/attach`, {
        method: "POST", token, body: { database_id },
      }),
    detach: (token: string, networkId: string, database_id: string) =>
      request<PrivateNetwork>(`/api/networks/${networkId}/detach`, {
        method: "POST", token, body: { database_id },
      }),
  },
  peerings: {
    list: (token: string) => request<NetworkPeering[]>("/api/peerings", { token }),
    get: (token: string, id: string) => request<NetworkPeering>(`/api/peerings/${id}`, { token }),
    create: (token: string, network_a_id: string, network_b_id: string) =>
      request<NetworkPeering>("/api/peerings", { method: "POST", token, body: { network_a_id, network_b_id } }),
    delete: (token: string, id: string) =>
      request<{ status: string }>(`/api/peerings/${id}`, { method: "DELETE", token }),
    addRule: (token: string, peeringId: string, rule: { action: string; source_network_id: string; dest_network_id: string; port?: number; protocol?: string; priority?: number; description?: string }) =>
      request<FirewallRule>(`/api/peerings/${peeringId}/rules`, { method: "POST", token, body: rule }),
    deleteRule: (token: string, peeringId: string, ruleId: string) =>
      request<{ status: string }>(`/api/peerings/${peeringId}/rules/${ruleId}`, { method: "DELETE", token }),
  },
  plans: {
    list: (token: string) => request<PlanTemplate[]>("/api/plans", { token }),
    listPublic: () => request<PlanTemplate[]>("/api/public/plans"),
  },
  billing: {
    periods: (token: string) => request<BillingPeriod[]>("/api/billing/periods", { token }),
    current: (token: string) => request<CurrentUsage>("/api/billing/current", { token }),
  },
  audit: {
    list: (token: string, page?: number, per_page?: number) =>
      request<AuditLog[]>(`/api/audit-logs?page=${page || 1}&per_page=${per_page || 50}`, { token }),
  },
  alerts: {
    list: (token: string) => request<AlertRule[]>("/api/alerts", { token }),
    create: (token: string, rule: { database_id?: string; event_type: string; webhook_url?: string; email?: string; enabled?: boolean }) =>
      request<AlertRule>("/api/alerts", { method: "POST", token, body: rule }),
    update: (token: string, id: string, data: { webhook_url?: string; email?: string; enabled?: boolean }) =>
      request<AlertRule>(`/api/alerts/${id}`, { method: "PUT", token, body: data }),
    delete: (token: string, id: string) =>
      request<{ status: string }>(`/api/alerts/${id}`, { method: "DELETE", token }),
    history: (token: string) => request<AlertHistory[]>("/api/alerts/history", { token }),
  },
  admin: {
    stats: (token: string) =>
      request<{ users: number; databases: number; registration_enabled: boolean }>("/api/admin/stats", { token }),
    listUsers: (token: string) => request<Array<Record<string, unknown>>>("/api/admin/users", { token }),
    updateUserRole: (token: string, userId: string, role: string) =>
      request("/api/admin/users/" + userId + "/role", { method: "PUT", token, body: { role } }),
    deleteUser: (token: string, userId: string) =>
      request("/api/admin/users/" + userId, { method: "DELETE", token }),
    listDatabases: (token: string) => request<Array<Record<string, unknown>>>("/api/admin/databases", { token }),
    forceDeleteDatabase: (token: string, id: string) =>
      request("/api/admin/databases/" + id, { method: "DELETE", token }),
    listInvitations: (token: string) => request<Array<Record<string, unknown>>>("/api/admin/invitations", { token }),
    createInvitation: (token: string, max_uses?: number, expires_in_hours?: number) =>
      request<Record<string, unknown>>("/api/admin/invitations", {
        method: "POST",
        token,
        body: { max_uses, expires_in_hours },
      }),
    deleteInvitation: (token: string, id: string) =>
      request("/api/admin/invitations/" + id, { method: "DELETE", token }),
    // Plan management
    listPlans: (token: string) => request<PlanTemplate[]>("/api/admin/plans", { token }),
    createPlan: (token: string, plan: Omit<PlanTemplate, "id" | "created_at">) =>
      request<PlanTemplate>("/api/admin/plans", { method: "POST", token, body: plan }),
    updatePlan: (token: string, id: string, plan: Partial<PlanTemplate>) =>
      request<PlanTemplate>("/api/admin/plans/" + id, { method: "PUT", token, body: plan }),
    deletePlan: (token: string, id: string) =>
      request("/api/admin/plans/" + id, { method: "DELETE", token }),
    billingOverview: (token: string) =>
      request<Record<string, unknown>>("/api/admin/billing/overview", { token }),
    generateBilling: (token: string) =>
      request<{ invoices_created: number }>("/api/admin/billing/generate", { method: "POST", token }),
  },
};
