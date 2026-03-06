"use client";

import { useState, useCallback } from "react";
import { useAuth } from "@/lib/auth";
import { api, AdminUser, Invitation } from "@/lib/api";
import { useAutoRefresh } from "@/lib/hooks";
import { useConfirm } from "@/components/ui/confirm-dialog";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { toast } from "sonner";
import { Trash2, ShieldCheck, User, Copy, Plus, Lock, Unlock, Ticket, X, Database, Radio, ChevronDown, ChevronRight, Cpu, MemoryStick } from "lucide-react";
import { UserResources } from "@/lib/api";

export default function UsersPage() {
  const { token } = useAuth();
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [invitations, setInvitations] = useState<Invitation[]>([]);
  const [regEnabled, setRegEnabled] = useState(true);
  const [showInviteForm, setShowInviteForm] = useState(false);
  const [inviteUses, setInviteUses] = useState(5);
  const [inviteHours, setInviteHours] = useState(168);
  const [tab, setTab] = useState<"users" | "invitations">("users");
  const { confirm, ConfirmDialog } = useConfirm();

  const load = useCallback(async () => {
    if (!token) return;
    try {
      const [u, inv, stats] = await Promise.all([
        api.admin.listUsers(token),
        api.admin.listInvitations(token),
        api.admin.stats(token),
      ]);
      setUsers(u);
      setInvitations(inv);
      setRegEnabled(stats.registration_enabled);
    } catch {}
  }, [token]);

  const { refreshing } = useAutoRefresh(load, 30000);

  const toggleReg = async () => {
    if (!token) return;
    try {
      const res = await api.admin.toggleRegistration(token, !regEnabled);
      setRegEnabled(res.registration_enabled);
      toast.success(res.registration_enabled ? "Inscriptions ouvertes" : "Inscriptions verrouillees — code requis");
    } catch { toast.error("Echec"); }
  };

  const toggleRole = async (id: string, role: string) => {
    if (!token) return;
    try {
      await api.admin.updateUserRole(token, id, role.toLowerCase() === "admin" ? "user" : "admin");
      toast.success("Role modifie");
      load();
    } catch { toast.error("Echec"); }
  };

  const delUser = async (id: string, email: string) => {
    if (!token) return;
    const ok = await confirm("Supprimer l'utilisateur", `Supprimer "${email}" et toutes ses bases de donnees ?`);
    if (!ok) return;
    try { await api.admin.deleteUser(token, id); toast.success("Supprime"); load(); }
    catch { toast.error("Echec"); }
  };

  const createInvite = async () => {
    if (!token) return;
    try {
      await api.admin.createInvitation(token, inviteUses, inviteHours);
      toast.success("Code cree");
      setShowInviteForm(false);
      load();
    } catch (err) { toast.error(err instanceof Error ? err.message : "Echec"); }
  };

  const delInvite = async (id: string) => {
    if (!token) return;
    try { await api.admin.deleteInvitation(token, id); toast.success("Code supprime"); load(); }
    catch { toast.error("Echec"); }
  };

  const copyCode = (code: string) => {
    navigator.clipboard.writeText(code);
    toast.success("Code copie !");
  };

  const admins = users.filter((u) => u.role.toLowerCase() === "admin");
  const regulars = users.filter((u) => u.role.toLowerCase() !== "admin");

  return (
    <div className="space-y-8">
      {ConfirmDialog}
      {/* Header with registration toggle */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Utilisateurs</h1>
          <p className="text-sm text-muted-foreground mt-1">{users.length} comptes · {admins.length} admins</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Radio className={`h-4 w-4 text-emerald-400 ${refreshing ? "animate-pulse" : ""}`} />
            Live — 30s
          </div>
          <Button
            size="sm"
            variant={regEnabled ? "outline" : "destructive"}
            className="gap-1.5 text-sm h-10"
            onClick={toggleReg}
          >
            {regEnabled ? <Unlock className="h-4 w-4" /> : <Lock className="h-4 w-4" />}
            {regEnabled ? "Inscriptions ouvertes" : "Inscriptions verrouillees"}
          </Button>
        </div>
      </div>

      {/* Registration info banner */}
      {!regEnabled && (
        <Card className="border-amber-500/20 bg-amber-500/5">
          <CardContent className="p-4 flex items-center gap-3">
            <Lock className="h-5 w-5 text-amber-400 shrink-0" />
            <p className="text-sm text-amber-200">
              Les inscriptions sont verrouillees. Seuls les utilisateurs avec un code d&apos;invitation valide peuvent s&apos;inscrire.
            </p>
          </CardContent>
        </Card>
      )}

      {/* Tabs */}
      <div className="flex gap-1 border-b border-border/50">
        <button onClick={() => setTab("users")} className={`px-4 py-2.5 text-base transition-colors border-b-2 -mb-px ${tab === "users" ? "border-primary text-foreground font-medium" : "border-transparent text-muted-foreground hover:text-foreground"}`}>
          <User className="h-4 w-4 inline mr-2" />Utilisateurs ({users.length})
        </button>
        <button onClick={() => setTab("invitations")} className={`px-4 py-2.5 text-base transition-colors border-b-2 -mb-px ${tab === "invitations" ? "border-primary text-foreground font-medium" : "border-transparent text-muted-foreground hover:text-foreground"}`}>
          <Ticket className="h-4 w-4 inline mr-2" />Codes d&apos;invitation ({invitations.length})
        </button>
      </div>

      {tab === "users" && (
        <div className="space-y-6">
          {/* Admins */}
          {admins.length > 0 && (
            <div>
              <p className="text-sm font-medium text-muted-foreground mb-3 flex items-center gap-2"><ShieldCheck className="h-4 w-4" />Administrateurs</p>
              <div className="grid gap-3 md:grid-cols-2">
                {admins.map((u) => <UserCard key={u.id} u={u} onToggle={toggleRole} onDelete={delUser} />)}
              </div>
            </div>
          )}
          {/* Users */}
          <div>
            <p className="text-sm font-medium text-muted-foreground mb-3">Utilisateurs</p>
            <div className="grid gap-3 md:grid-cols-2">
              {regulars.map((u) => <UserCard key={u.id} u={u} onToggle={toggleRole} onDelete={delUser} />)}
            </div>
            {regulars.length === 0 && <p className="text-base text-muted-foreground text-center py-8">Aucun utilisateur</p>}
          </div>
        </div>
      )}

      {tab === "invitations" && (
        <div className="space-y-4">
          <div className="flex justify-end">
            <Button size="sm" className="gap-1.5 h-10" onClick={() => setShowInviteForm(!showInviteForm)}>
              {showInviteForm ? <X className="h-4 w-4" /> : <Plus className="h-4 w-4" />}
              {showInviteForm ? "Annuler" : "Nouveau code"}
            </Button>
          </div>

          {showInviteForm && (
            <Card className="border-primary/20">
              <CardContent className="p-5">
                <div className="grid grid-cols-3 gap-4">
                  <div>
                    <p className="text-sm text-muted-foreground mb-1">Utilisations max</p>
                    <Input className="h-10 text-base" type="number" min={1} value={inviteUses} onChange={(e) => setInviteUses(parseInt(e.target.value) || 1)} />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground mb-1">Expire dans (heures)</p>
                    <Input className="h-10 text-base" type="number" min={1} value={inviteHours} onChange={(e) => setInviteHours(parseInt(e.target.value) || 24)} />
                    <p className="text-xs text-muted-foreground mt-1">{(inviteHours / 24).toFixed(0)}j</p>
                  </div>
                  <div className="flex items-end">
                    <Button size="sm" className="w-full h-10 text-sm" onClick={createInvite}>Generer</Button>
                  </div>
                </div>
              </CardContent>
            </Card>
          )}

          <div className="space-y-3">
            {invitations.map((inv) => {
              const expired = inv.expires_at && new Date(inv.expires_at) < new Date();
              const exhausted = inv.use_count >= inv.max_uses;
              const valid = !expired && !exhausted;
              return (
                <Card key={inv.id} className={!valid ? "opacity-50" : ""}>
                  <CardContent className="p-4 flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <Ticket className="h-5 w-5 text-muted-foreground" />
                      <div>
                        <div className="flex items-center gap-2">
                          <code className="text-base font-mono font-medium bg-muted/50 px-2.5 py-1 rounded">{inv.code}</code>
                          <Button variant="ghost" size="sm" className="h-7 w-7 p-0" onClick={() => copyCode(inv.code)}><Copy className="h-3.5 w-3.5" /></Button>
                        </div>
                        <p className="text-xs text-muted-foreground mt-1">
                          {inv.use_count}/{inv.max_uses} utilisations
                          {inv.expires_at && ` · expire ${new Date(inv.expires_at).toLocaleDateString("fr")}`}
                        </p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {expired && <Badge variant="outline" className="text-xs text-red-400 border-red-400/20">Expire</Badge>}
                      {exhausted && <Badge variant="outline" className="text-xs text-amber-400 border-amber-400/20">Epuise</Badge>}
                      {valid && <Badge variant="outline" className="text-xs text-emerald-400 border-emerald-400/20">Actif</Badge>}
                      <Button variant="ghost" size="sm" className="h-8 w-8 p-0 text-destructive" onClick={() => delInvite(inv.id)}><Trash2 className="h-4 w-4" /></Button>
                    </div>
                  </CardContent>
                </Card>
              );
            })}
            {invitations.length === 0 && (
              <p className="text-base text-muted-foreground text-center py-8">Aucun code d&apos;invitation</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function UserCard({ u, onToggle, onDelete }: { u: AdminUser; onToggle: (id: string, role: string) => void; onDelete: (id: string, email: string) => void }) {
  const { token } = useAuth();
  const isAdmin = u.role.toLowerCase() === "admin";
  const [expanded, setExpanded] = useState(false);
  const [resources, setResources] = useState<UserResources | null>(null);

  const toggleExpand = async () => {
    if (!expanded && !resources && token) {
      try {
        const res = await api.admin.userResources(token, u.id);
        setResources(res);
      } catch {}
    }
    setExpanded(!expanded);
  };

  return (
    <Card className="group hover:border-border transition-colors">
      <CardContent className="p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3 min-w-0">
            <div className={`h-10 w-10 rounded-full flex items-center justify-center shrink-0 ${isAdmin ? "bg-amber-500/10" : "bg-accent"}`}>
              {isAdmin ? <ShieldCheck className="h-4 w-4 text-amber-400" /> : <User className="h-4 w-4 text-muted-foreground" />}
            </div>
            <div className="min-w-0">
              <p className="text-base font-medium truncate">{u.email}</p>
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <span className="flex items-center gap-1"><Database className="h-3 w-3" />{u.database_count} DBs</span>
                <span>max {u.max_databases}</span>
                <span>{new Date(u.created_at).toLocaleDateString("fr")}</span>
              </div>
            </div>
          </div>
          <div className="flex items-center gap-1">
            {u.database_count > 0 && (
              <Button variant="ghost" size="sm" className="h-8 w-8 p-0" onClick={toggleExpand}>
                {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
              </Button>
            )}
            <div className="opacity-0 group-hover:opacity-100 transition-opacity flex items-center gap-1">
              <Button variant="outline" size="sm" className="h-8 text-xs px-3" onClick={() => onToggle(u.id, u.role)}>
                {isAdmin ? "Demote" : "Promote"}
              </Button>
              <Button variant="ghost" size="sm" className="h-8 w-8 p-0 text-destructive" onClick={() => onDelete(u.id, u.email)}>
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </div>
        {expanded && resources && (
          <div className="mt-3 pt-3 border-t border-border/30 space-y-2">
            <div className="flex items-center gap-4 text-xs text-muted-foreground">
              <span className="flex items-center gap-1"><Cpu className="h-3 w-3" /> {resources.total_cpu} vCPU</span>
              <span className="flex items-center gap-1"><MemoryStick className="h-3 w-3" /> {resources.total_memory_mb} MB RAM</span>
            </div>
            {resources.databases.map((db) => (
              <div key={db.id} className="flex items-center justify-between text-xs bg-accent/30 rounded px-3 py-1.5">
                <div className="flex items-center gap-2">
                  <Badge variant="outline" className="text-[10px] px-1.5">{db.db_type}</Badge>
                  <span className="font-medium">{db.name}</span>
                </div>
                <div className="flex items-center gap-3 text-muted-foreground">
                  <span>{db.cpu_limit} CPU</span>
                  <span>{db.memory_limit_mb} MB</span>
                  <Badge variant="outline" className={`text-[10px] ${db.status === "running" ? "text-emerald-400 border-emerald-500/20" : "text-zinc-400"}`}>
                    {db.status}
                  </Badge>
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
