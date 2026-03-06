"use client";

import { useEffect, useState } from "react";
import { useAuth } from "@/lib/auth";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";

export default function AdminDatabasesPage() {
  const { token } = useAuth();
  const [dbs, setDbs] = useState<Array<Record<string, unknown>>>([]);

  const fetchDbs = async () => {
    if (!token) return;
    try {
      setDbs(await api.admin.listDatabases(token));
    } catch {
      toast.error("Failed to load databases");
    }
  };

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    fetchDbs();
  }, [token]); // eslint-disable-line react-hooks/exhaustive-deps

  const forceDelete = async (id: string) => {
    if (!token || !confirm("Force delete this database?")) return;
    try {
      await api.admin.forceDeleteDatabase(token, id);
      toast.success("Database deleted");
      fetchDbs();
    } catch {
      toast.error("Failed to delete database");
    }
  };

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">All Databases</h1>
      <div className="space-y-2">
        {dbs.map((db) => (
          <div key={db.id as string} className="flex items-center justify-between bg-card border border-border rounded-lg px-4 py-3">
            <div className="flex items-center gap-3">
              <span className="text-sm font-medium">{db.name as string}</span>
              <Badge variant="outline">{(db.db_type as string) === "postgresql" ? "PG" : "Redis"}</Badge>
              <Badge>{db.status as string}</Badge>
              <span className="text-xs text-muted-foreground">Port {db.port as number}</span>
              <span className="text-xs text-muted-foreground">User: {(db.user_id as string).slice(0, 8)}...</span>
            </div>
            <Button size="sm" variant="destructive" onClick={() => forceDelete(db.id as string)}>
              Force Delete
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
