const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:3001";

interface RequestOptions {
  method?: string;
  body?: unknown;
  token?: string;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const { method = "GET", body, token } = options;
  const headers: Record<string, string> = {};
  if (token) headers["Authorization"] = `Bearer ${token}`;
  if (body) headers["Content-Type"] = "application/json";

  const res = await fetch(`${API_BASE}${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!res.ok) {
    const error = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(error.error || "Request failed");
  }

  const text = await res.text();
  return text ? JSON.parse(text) : ({} as T);
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

export interface BillingPeriod {
  id: string;
  user_id: string;
  period_start: string;
  period_end: string;
  total_cents: number;
  stripe_invoice_id: string | null;
  status: string;
  created_at: string;
}

export interface DockerServerStatus {
  id: string;
  name: string;
  url: string;
  region: string | null;
  active: boolean;
  server_type: string;
  max_containers: number;
  online: boolean;
  containers_running: number | null;
  containers_total: number | null;
  cpu_count: number | null;
  memory_bytes: number | null;
  docker_version: string | null;
  last_seen_at: string | null;
  error: string | null;
}

export interface AdminStats {
  users: number;
  databases: number;
  registration_enabled: boolean;
  user_growth: Array<{ date: string; count: number }>;
  db_growth: Array<{ date: string; count: number }>;
  status_breakdown: Array<{ status: string; count: number }>;
  type_breakdown: Array<{ type: string; count: number }>;
  revenue_monthly: Array<{ month: string; total: number }>;
}

export interface AdminUser {
  id: string;
  email: string;
  role: string;
  max_databases: number;
  database_count: number;
  created_at: string;
}

export interface AdminDatabase {
  id: string;
  user_id: string;
  user_email: string;
  name: string;
  db_type: string;
  status: string;
  port: number;
  cpu_limit: number;
  memory_limit_mb: number;
  plan_template_id: string | null;
  bundle_id: string | null;
  docker_server_id: string | null;
  server_name: string;
  subdomain?: string;
  routing_mode?: string;
  created_at: string;
}

export interface Invitation {
  id: string;
  code: string;
  max_uses: number;
  use_count: number;
  expires_at: string | null;
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

export interface SystemHealth {
  servers: Array<{
    id: string;
    name: string;
    url: string;
    online: boolean;
    server_type: string;
  }>;
  databases: Array<{
    id: string;
    name: string;
    db_type: string;
    status: string;
    docker_server_id: string | null;
  }>;
  maintenance_mode: boolean;
}

export interface UserResources {
  user_id: string;
  total_cpu: number;
  total_memory_mb: number;
  databases: Array<{
    id: string;
    name: string;
    db_type: string;
    status: string;
    cpu_limit: number;
    memory_limit_mb: number;
    plan_template_id: string | null;
  }>;
}

export interface ServerContainer {
  id: string;
  name: string;
  image: string;
  state: string;
  cpu_percent: number;
  memory_usage_bytes: number;
  memory_limit_bytes: number;
  is_dbaas: boolean;
}

export interface ServerResources {
  cpu_count: number | null;
  memory_total_bytes: number | null;
  containers_running: number | null;
  containers_stopped: number | null;
  containers_total: number | null;
  images_count: number;
  volumes_count: number;
  images_size_bytes: number;
  containers_size_bytes: number;
  os: string | null;
  kernel: string | null;
  docker_root_dir: string | null;
}

export const api = {
  auth: {
    login: (email: string, password: string) =>
      request<{ token: string; user: { id: string; email: string; role: string } }>("/api/auth/login", {
        method: "POST",
        body: { email, password },
      }),
    me: (token: string) => request<{ id: string; email: string; role: string }>("/api/auth/me", { token }),
  },
  admin: {
    stats: (token: string) => request<AdminStats>("/api/admin/stats", { token }),

    // Settings
    toggleRegistration: (token: string, enabled: boolean) =>
      request<{ registration_enabled: boolean }>("/api/admin/settings/registration", {
        method: "PUT", token, body: { enabled },
      }),

    // Users
    listUsers: (token: string) => request<AdminUser[]>("/api/admin/users", { token }),
    updateUserRole: (token: string, userId: string, role: string) =>
      request("/api/admin/users/" + userId + "/role", { method: "PUT", token, body: { role } }),
    deleteUser: (token: string, userId: string) =>
      request("/api/admin/users/" + userId, { method: "DELETE", token }),

    // Databases
    listDatabases: (token: string) => request<AdminDatabase[]>("/api/admin/databases", { token }),
    forceDeleteDatabase: (token: string, id: string) =>
      request("/api/admin/databases/" + id, { method: "DELETE", token }),
    migrateSni: (token: string, id: string) =>
      request("/api/admin/databases/" + id + "/migrate-sni", { method: "POST", token }),

    // Plans
    listPlans: (token: string) => request<PlanTemplate[]>("/api/admin/plans", { token }),
    createPlan: (token: string, plan: Omit<PlanTemplate, "id" | "created_at">) =>
      request<PlanTemplate>("/api/admin/plans", { method: "POST", token, body: plan }),
    updatePlan: (token: string, id: string, plan: Partial<PlanTemplate>) =>
      request<PlanTemplate>("/api/admin/plans/" + id, { method: "PUT", token, body: plan }),
    deletePlan: (token: string, id: string) =>
      request("/api/admin/plans/" + id, { method: "DELETE", token }),

    // Invitations
    listInvitations: (token: string) => request<Invitation[]>("/api/admin/invitations", { token }),
    createInvitation: (token: string, max_uses?: number, expires_in_hours?: number) =>
      request<Invitation>("/api/admin/invitations", { method: "POST", token, body: { max_uses, expires_in_hours } }),
    deleteInvitation: (token: string, id: string) =>
      request("/api/admin/invitations/" + id, { method: "DELETE", token }),

    // Billing
    billingOverview: (token: string) =>
      request<{ total_revenue_cents: number; pending_revenue_cents: number; total_periods: number; periods: BillingPeriod[] }>(
        "/api/admin/billing/overview", { token }
      ),
    generateBilling: (token: string) =>
      request<{ invoices_created: number }>("/api/admin/billing/generate", { method: "POST", token }),

    // Private Networks
    listNetworks: (token: string) =>
      request<Array<{
        id: string;
        user_id: string;
        name: string;
        docker_server_id: string | null;
        subnet: string | null;
        gateway: string | null;
        member_count: number;
        members: Array<{
          database_id: string;
          database_name: string;
          db_type: string;
          hostname: string;
          port: number;
          joined_at: string;
        }>;
        created_at: string;
      }>>("/api/admin/networks", { token }),

    // Docker Servers
    listServers: (token: string) => request<DockerServerStatus[]>("/api/admin/servers/status", { token }),
    createServer: (token: string, server: { name: string; url: string; region?: string; max_containers?: number; notes?: string; server_type?: string; tls_ca?: string; tls_cert?: string; tls_key?: string }) =>
      request("/api/admin/servers", { method: "POST", token, body: server }),
    updateServer: (token: string, id: string, data: Record<string, unknown>) =>
      request("/api/admin/servers/" + id, { method: "PUT", token, body: data }),
    deleteServer: (token: string, id: string) =>
      request("/api/admin/servers/" + id, { method: "DELETE", token }),
    serverContainers: (token: string, id: string) =>
      request<ServerContainer[]>("/api/admin/servers/" + id + "/containers", { token }),
    serverResources: (token: string, id: string) =>
      request<ServerResources>("/api/admin/servers/" + id + "/resources", { token }),

    // Audit Logs
    auditLogs: (token: string, page?: number, per_page?: number, action?: string, resource_type?: string) => {
      const params = new URLSearchParams();
      if (page) params.set("page", String(page));
      if (per_page) params.set("per_page", String(per_page));
      if (action) params.set("action", action);
      if (resource_type) params.set("resource_type", resource_type);
      return request<AuditLog[]>(`/api/admin/audit-logs?${params}`, { token });
    },

    // System Health
    systemHealth: (token: string) => request<SystemHealth>("/api/admin/health", { token }),

    // Maintenance Mode
    toggleMaintenance: (token: string, enabled: boolean) =>
      request<{ maintenance_mode: boolean }>("/api/admin/settings/maintenance", {
        method: "PUT", token, body: { enabled },
      }),

    // User Resources
    userResources: (token: string, userId: string) =>
      request<UserResources>(`/api/admin/users/${userId}/resources`, { token }),
  },
};
