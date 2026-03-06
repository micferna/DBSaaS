"use client";

import { useEffect, useState } from "react";
import { useAuth } from "@/lib/auth";
import { api } from "@/lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export default function AdminPage() {
  const { token } = useAuth();
  const [stats, setStats] = useState<{ users: number; databases: number; registration_enabled: boolean } | null>(null);

  useEffect(() => {
    if (!token) return;
    api.admin.stats(token).then(setStats).catch(() => {});
  }, [token]);

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Admin Overview</h1>
      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm text-muted-foreground">Total Users</CardTitle></CardHeader>
          <CardContent><p className="text-3xl font-bold">{stats?.users ?? "..."}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm text-muted-foreground">Total Databases</CardTitle></CardHeader>
          <CardContent><p className="text-3xl font-bold">{stats?.databases ?? "..."}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm text-muted-foreground">Registration</CardTitle></CardHeader>
          <CardContent><p className="text-3xl font-bold">{stats?.registration_enabled ? "Open" : "Closed"}</p></CardContent>
        </Card>
      </div>
    </div>
  );
}
