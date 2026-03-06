"use client";

import { useEffect, useState, useCallback } from "react";
import { useAuth } from "@/lib/auth";
import { api, PrivateNetwork, DatabaseInstance, NetworkPeering, FirewallRule } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { toast } from "sonner";
import {
  Network,
  Plus,
  Trash2,
  Unplug,
  Link2,
  ChevronDown,
  ChevronRight,
  Database,
  Loader2,
  Shield,
  ArrowRightLeft,
} from "lucide-react";

export default function NetworksPage() {
  const { token } = useAuth();
  const [networks, setNetworks] = useState<PrivateNetwork[]>([]);
  const [databases, setDatabases] = useState<DatabaseInstance[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [peerings, setPeerings] = useState<NetworkPeering[]>([]);
  const [showCreatePeering, setShowCreatePeering] = useState(false);
  const [peeringNetA, setPeeringNetA] = useState("");
  const [peeringNetB, setPeeringNetB] = useState("");
  const [creatingPeering, setCreatingPeering] = useState(false);
  const [expandedPeerings, setExpandedPeerings] = useState<Set<string>>(new Set());
  const [showAddRule, setShowAddRule] = useState<string | null>(null);
  const [rulePort, setRulePort] = useState("");
  const [ruleDirection, setRuleDirection] = useState<"a_to_b" | "b_to_a">("a_to_b");

  const load = useCallback(async () => {
    if (!token) return;
    try {
      const [nets, dbs] = await Promise.all([
        api.networks.list(token),
        api.databases.list(token),
      ]);
      setNetworks(nets);
      setDatabases(dbs);
    } catch {
      toast.error("Failed to load networks");
    }
    // Load peerings separately so it doesn't break networks if table doesn't exist yet
    try {
      const peers = await api.peerings.list(token);
      setPeerings(peers);
    } catch {
      // peerings table may not exist yet
    }
    setLoading(false);
  }, [token]);

  useEffect(() => {
    load();
    const interval = setInterval(load, 5000);
    return () => clearInterval(interval);
  }, [load]);

  const toggleExpand = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleCreate = async () => {
    if (!token || !newName.trim()) return;
    setCreating(true);
    try {
      await api.networks.create(token, newName.trim());
      setNewName("");
      setShowCreate(false);
      toast.success("Network created");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create network");
    } finally {
      setCreating(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!token) return;
    setActionLoading(id);
    try {
      await api.networks.delete(token, id);
      toast.success("Network deleted");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete network");
    } finally {
      setActionLoading(null);
    }
  };

  const handleAttach = async (networkId: string, databaseId: string) => {
    if (!token) return;
    setActionLoading(`attach-${networkId}-${databaseId}`);
    try {
      await api.networks.attach(token, networkId, databaseId);
      toast.success("Database attached");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to attach database");
    } finally {
      setActionLoading(null);
    }
  };

  const handleDetach = async (networkId: string, databaseId: string) => {
    if (!token) return;
    setActionLoading(`detach-${networkId}-${databaseId}`);
    try {
      await api.networks.detach(token, networkId, databaseId);
      toast.success("Database detached");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to detach database");
    } finally {
      setActionLoading(null);
    }
  };

  const togglePeeringExpand = (id: string) => {
    setExpandedPeerings((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleCreatePeering = async () => {
    if (!token || !peeringNetA || !peeringNetB) return;
    setCreatingPeering(true);
    try {
      await api.peerings.create(token, peeringNetA, peeringNetB);
      setPeeringNetA("");
      setPeeringNetB("");
      setShowCreatePeering(false);
      toast.success("Peering created");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create peering");
    } finally {
      setCreatingPeering(false);
    }
  };

  const handleDeletePeering = async (id: string) => {
    if (!token) return;
    setActionLoading(`peering-${id}`);
    try {
      await api.peerings.delete(token, id);
      toast.success("Peering deleted");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete peering");
    } finally {
      setActionLoading(null);
    }
  };

  const handleAddRule = async (peering: NetworkPeering) => {
    if (!token || !rulePort) return;
    const port = parseInt(rulePort, 10);
    if (isNaN(port) || port <= 0 || port >= 65536) {
      toast.error("Invalid port number");
      return;
    }
    setActionLoading(`rule-add-${peering.id}`);
    try {
      const srcId = ruleDirection === "a_to_b" ? peering.network_a.id : peering.network_b.id;
      const dstId = ruleDirection === "a_to_b" ? peering.network_b.id : peering.network_a.id;
      await api.peerings.addRule(token, peering.id, {
        action: "allow",
        source_network_id: srcId,
        dest_network_id: dstId,
        port,
        protocol: "tcp",
      });
      setRulePort("");
      setShowAddRule(null);
      toast.success("Firewall rule added");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to add rule");
    } finally {
      setActionLoading(null);
    }
  };

  const handleDeleteRule = async (peeringId: string, ruleId: string) => {
    if (!token) return;
    setActionLoading(`rule-del-${ruleId}`);
    try {
      await api.peerings.deleteRule(token, peeringId, ruleId);
      toast.success("Firewall rule deleted");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete rule");
    } finally {
      setActionLoading(null);
    }
  };

  const getEligibleDatabases = (network: PrivateNetwork) => {
    const memberIds = new Set(network.members.map((m) => m.database_id));
    return databases.filter(
      (db) =>
        db.status === "running" &&
        !memberIds.has(db.id)
    );
  };

  const dbTypePort = (dbType: string) => {
    switch (dbType) {
      case "postgresql": return 5432;
      case "redis": return 6379;
      case "mariadb": return 3306;
      default: return 0;
    }
  };

  const dbTypeColor = (dbType: string) => {
    switch (dbType) {
      case "postgresql": return "bg-blue-500/10 text-blue-600";
      case "redis": return "bg-red-500/10 text-red-600";
      case "mariadb": return "bg-emerald-500/10 text-emerald-600";
      default: return "";
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">Private Networks</h1>
        <Button size="sm" onClick={() => setShowCreate(!showCreate)}>
          <Plus className="h-4 w-4 mr-1" /> New Network
        </Button>
      </div>

      {showCreate && (
        <Card>
          <CardContent className="pt-6">
            <div className="flex gap-3">
              <Input
                placeholder="Network name"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleCreate()}
                className="max-w-xs"
              />
              <Button onClick={handleCreate} disabled={creating || !newName.trim()}>
                {creating ? <Loader2 className="h-4 w-4 animate-spin" /> : "Create"}
              </Button>
              <Button variant="ghost" onClick={() => setShowCreate(false)}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {networks.length === 0 && !showCreate ? (
        <Card>
          <CardContent className="py-12 text-center">
            <Network className="h-10 w-10 mx-auto text-muted-foreground mb-3" />
            <p className="text-sm text-muted-foreground">
              No private networks yet. Create one to connect your databases together.
            </p>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-3">
          {networks.map((net) => {
            const isExpanded = expanded.has(net.id);
            const eligible = getEligibleDatabases(net);

            return (
              <Card key={net.id}>
                <CardHeader
                  className="pb-3 cursor-pointer"
                  onClick={() => toggleExpand(net.id)}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      {isExpanded ? (
                        <ChevronDown className="h-4 w-4 text-muted-foreground" />
                      ) : (
                        <ChevronRight className="h-4 w-4 text-muted-foreground" />
                      )}
                      <CardTitle className="text-sm flex items-center gap-2">
                        <Network className="h-4 w-4 text-muted-foreground" />
                        {net.name}
                      </CardTitle>
                      <Badge variant="secondary" className="text-xs">
                        {net.members.length} member{net.members.length !== 1 ? "s" : ""}
                      </Badge>
                      {net.subnet && (
                        <code className="text-[11px] bg-muted px-1.5 py-0.5 rounded font-mono text-muted-foreground">
                          {net.subnet}
                        </code>
                      )}
                      {peerings.filter(p => p.network_a.id === net.id || p.network_b.id === net.id).length > 0 && (
                        <Badge variant="secondary" className="text-xs bg-violet-500/10 text-violet-600">
                          <ArrowRightLeft className="h-3 w-3 mr-1" />
                          {peerings.filter(p => p.network_a.id === net.id || p.network_b.id === net.id).length} peering{peerings.filter(p => p.network_a.id === net.id || p.network_b.id === net.id).length > 1 ? "s" : ""}
                        </Badge>
                      )}
                    </div>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7 text-destructive hover:text-destructive"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(net.id);
                      }}
                      disabled={actionLoading === net.id}
                    >
                      {actionLoading === net.id ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      ) : (
                        <Trash2 className="h-3.5 w-3.5" />
                      )}
                    </Button>
                  </div>
                </CardHeader>

                {isExpanded && (
                  <CardContent className="pt-0 space-y-4">
                    {/* Network Info */}
                    {(net.subnet || net.gateway) && (
                      <div className="grid grid-cols-2 gap-3">
                        {net.subnet && (
                          <div className="rounded-lg bg-muted/30 border border-border/30 p-3">
                            <p className="text-[10px] text-muted-foreground uppercase tracking-wider mb-1">Subnet</p>
                            <code className="text-sm font-mono">{net.subnet}</code>
                          </div>
                        )}
                        {net.gateway && (
                          <div className="rounded-lg bg-muted/30 border border-border/30 p-3">
                            <p className="text-[10px] text-muted-foreground uppercase tracking-wider mb-1">Gateway</p>
                            <code className="text-sm font-mono">{net.gateway}</code>
                          </div>
                        )}
                      </div>
                    )}

                    {/* Members */}
                    {net.members.length > 0 && (
                      <div className="space-y-2">
                        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                          Members
                        </p>
                        <div className="space-y-1.5">
                          {net.members.map((member) => (
                            <div
                              key={member.database_id}
                              className="flex items-center justify-between rounded-lg border px-3 py-2"
                            >
                              <div className="flex items-center gap-3">
                                <Database className="h-3.5 w-3.5 text-muted-foreground" />
                                <span className="text-sm font-medium">
                                  {member.database_name}
                                </span>
                                <Badge
                                  variant="secondary"
                                  className={`text-xs ${dbTypeColor(member.db_type)}`}
                                >
                                  {member.db_type}
                                </Badge>
                                <code className="text-xs bg-muted px-1.5 py-0.5 rounded font-mono">
                                  {member.hostname}:{dbTypePort(member.db_type)}
                                </code>
                              </div>
                              <Button
                                variant="ghost"
                                size="sm"
                                className="h-7 text-muted-foreground hover:text-destructive"
                                onClick={() => handleDetach(net.id, member.database_id)}
                                disabled={
                                  actionLoading === `detach-${net.id}-${member.database_id}`
                                }
                              >
                                {actionLoading ===
                                `detach-${net.id}-${member.database_id}` ? (
                                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                ) : (
                                  <Unplug className="h-3.5 w-3.5" />
                                )}
                              </Button>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Connection info */}
                    {net.members.length >= 2 && (
                      <div className="rounded-lg bg-muted/50 p-3">
                        <p className="text-xs text-muted-foreground">
                          <Link2 className="h-3 w-3 inline mr-1" />
                          Databases in this network can reach each other via their internal
                          hostname (e.g.{" "}
                          <code className="bg-muted px-1 rounded">
                            {net.members[0].hostname}:{dbTypePort(net.members[0].db_type)}
                          </code>
                          ).
                        </p>
                      </div>
                    )}

                    {/* Attach dropdown */}
                    {eligible.length > 0 && (
                      <div className="space-y-2">
                        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                          Attach Database
                        </p>
                        <div className="flex flex-wrap gap-2">
                          {eligible.map((db) => (
                            <Button
                              key={db.id}
                              variant="outline"
                              size="sm"
                              className="h-7 text-xs"
                              onClick={() => handleAttach(net.id, db.id)}
                              disabled={actionLoading === `attach-${net.id}-${db.id}`}
                            >
                              {actionLoading === `attach-${net.id}-${db.id}` ? (
                                <Loader2 className="h-3 w-3 animate-spin mr-1" />
                              ) : (
                                <Plus className="h-3 w-3 mr-1" />
                              )}
                              {db.name}
                              <Badge
                                variant="secondary"
                                className={`text-xs ml-1 ${dbTypeColor(db.db_type)}`}
                              >
                                {db.db_type}
                              </Badge>
                            </Button>
                          ))}
                        </div>
                      </div>
                    )}

                    {eligible.length === 0 && net.members.length === 0 && (
                      <p className="text-xs text-muted-foreground">
                        No eligible databases. Create a running database first.
                      </p>
                    )}
                  </CardContent>
                )}
              </Card>
            );
          })}
        </div>
      )}

      {/* Network Peerings Section */}
      <div className="border-t pt-6 mt-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold tracking-tight flex items-center gap-2">
            <ArrowRightLeft className="h-5 w-5 text-muted-foreground" />
            Network Peerings
          </h2>
          <Button
            size="sm"
            variant="outline"
            onClick={() => setShowCreatePeering(!showCreatePeering)}
            disabled={networks.length < 2}
            title={networks.length < 2 ? "You need at least 2 networks to create a peering" : ""}
          >
            <Plus className="h-4 w-4 mr-1" /> New Peering
          </Button>
        </div>

        {showCreatePeering && (
          <Card className="mb-4">
            <CardContent className="pt-6">
              <div className="flex gap-3 items-end flex-wrap">
                <div className="space-y-1">
                  <label className="text-xs text-muted-foreground">Network A</label>
                  <select
                    className="block w-48 rounded-md border border-input bg-background px-3 py-1.5 text-sm"
                    value={peeringNetA}
                    onChange={(e) => setPeeringNetA(e.target.value)}
                  >
                    <option value="">Select network...</option>
                    {networks.map((n) => (
                      <option key={n.id} value={n.id}>{n.name}</option>
                    ))}
                  </select>
                </div>
                <span className="text-muted-foreground pb-1">↔</span>
                <div className="space-y-1">
                  <label className="text-xs text-muted-foreground">Network B</label>
                  <select
                    className="block w-48 rounded-md border border-input bg-background px-3 py-1.5 text-sm"
                    value={peeringNetB}
                    onChange={(e) => setPeeringNetB(e.target.value)}
                  >
                    <option value="">Select network...</option>
                    {networks.filter((n) => n.id !== peeringNetA).map((n) => (
                      <option key={n.id} value={n.id}>{n.name}</option>
                    ))}
                  </select>
                </div>
                <Button
                  onClick={handleCreatePeering}
                  disabled={creatingPeering || !peeringNetA || !peeringNetB}
                  size="sm"
                >
                  {creatingPeering ? <Loader2 className="h-4 w-4 animate-spin" /> : "Create"}
                </Button>
                <Button variant="ghost" size="sm" onClick={() => setShowCreatePeering(false)}>
                  Cancel
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        {peerings.length === 0 ? (
          <Card>
            <CardContent className="py-8 text-center">
              <ArrowRightLeft className="h-8 w-8 mx-auto text-muted-foreground mb-3" />
              <p className="text-sm text-muted-foreground">
                No peerings yet. Connect two networks to allow controlled traffic between them.
              </p>
            </CardContent>
          </Card>
        ) : (
          <div className="space-y-3">
            {peerings.map((peering) => {
              const isPeeringExpanded = expandedPeerings.has(peering.id);
              const netAName = peering.network_a.name;
              const netBName = peering.network_b.name;

              return (
                <Card key={peering.id}>
                  <CardHeader
                    className="pb-3 cursor-pointer"
                    onClick={() => togglePeeringExpand(peering.id)}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        {isPeeringExpanded ? (
                          <ChevronDown className="h-4 w-4 text-muted-foreground" />
                        ) : (
                          <ChevronRight className="h-4 w-4 text-muted-foreground" />
                        )}
                        <CardTitle className="text-sm flex items-center gap-2">
                          <ArrowRightLeft className="h-4 w-4 text-muted-foreground" />
                          {netAName} ↔ {netBName}
                        </CardTitle>
                        <Badge
                          variant="secondary"
                          className={`text-xs ${
                            peering.status === "active"
                              ? "bg-green-500/10 text-green-600"
                              : peering.status === "pending"
                              ? "bg-yellow-500/10 text-yellow-600"
                              : "bg-red-500/10 text-red-600"
                          }`}
                        >
                          {peering.status}
                        </Badge>
                        {peering.rules.length > 0 && (
                          <Badge variant="secondary" className="text-xs">
                            {peering.rules.length} rule{peering.rules.length !== 1 ? "s" : ""}
                          </Badge>
                        )}
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 text-destructive hover:text-destructive"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDeletePeering(peering.id);
                        }}
                        disabled={actionLoading === `peering-${peering.id}`}
                      >
                        {actionLoading === `peering-${peering.id}` ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />
                        ) : (
                          <Trash2 className="h-3.5 w-3.5" />
                        )}
                      </Button>
                    </div>
                  </CardHeader>

                  {isPeeringExpanded && (
                    <CardContent className="pt-0 space-y-4">
                      {/* Default deny notice */}
                      <div className="rounded-lg bg-amber-500/5 border border-amber-500/20 p-3">
                        <p className="text-xs text-amber-700 dark:text-amber-400">
                          <Shield className="h-3 w-3 inline mr-1" />
                          Default policy: <strong>deny all</strong>. Only explicitly allowed ports will pass traffic.
                        </p>
                      </div>

                      {/* Firewall Rules */}
                      {peering.rules.length > 0 && (
                        <div className="space-y-2">
                          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                            Firewall Rules
                          </p>
                          <div className="space-y-1.5">
                            {peering.rules.map((rule) => {
                              const srcName = rule.source_network_id === peering.network_a.id ? netAName : netBName;
                              const dstName = rule.dest_network_id === peering.network_a.id ? netAName : netBName;
                              return (
                                <div
                                  key={rule.id}
                                  className="flex items-center justify-between rounded-lg border px-3 py-2"
                                >
                                  <div className="flex items-center gap-3 text-sm">
                                    <Badge
                                      variant="secondary"
                                      className={`text-xs ${
                                        rule.action === "allow"
                                          ? "bg-green-500/10 text-green-600"
                                          : "bg-red-500/10 text-red-600"
                                      }`}
                                    >
                                      {rule.action.toUpperCase()}
                                    </Badge>
                                    <span className="text-muted-foreground">{srcName}</span>
                                    <span className="text-muted-foreground">→</span>
                                    <span className="text-muted-foreground">{dstName}</span>
                                    {rule.port && (
                                      <code className="text-xs bg-muted px-1.5 py-0.5 rounded font-mono">
                                        :{rule.port}
                                      </code>
                                    )}
                                    <span className="text-xs text-muted-foreground">
                                      {rule.protocol || "tcp"}
                                    </span>
                                    {rule.description && (
                                      <span className="text-xs text-muted-foreground italic">
                                        {rule.description}
                                      </span>
                                    )}
                                  </div>
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    className="h-7 text-muted-foreground hover:text-destructive"
                                    onClick={() => handleDeleteRule(peering.id, rule.id)}
                                    disabled={actionLoading === `rule-del-${rule.id}`}
                                  >
                                    {actionLoading === `rule-del-${rule.id}` ? (
                                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                    ) : (
                                      <Trash2 className="h-3.5 w-3.5" />
                                    )}
                                  </Button>
                                </div>
                              );
                            })}
                          </div>
                        </div>
                      )}

                      {/* Add Rule */}
                      {showAddRule === peering.id ? (
                        <div className="flex gap-3 items-end flex-wrap">
                          <div className="space-y-1">
                            <label className="text-xs text-muted-foreground">Direction</label>
                            <select
                              className="block w-56 rounded-md border border-input bg-background px-3 py-1.5 text-sm"
                              value={ruleDirection}
                              onChange={(e) => setRuleDirection(e.target.value as "a_to_b" | "b_to_a")}
                            >
                              <option value="a_to_b">{netAName} → {netBName}</option>
                              <option value="b_to_a">{netBName} → {netAName}</option>
                            </select>
                          </div>
                          <div className="space-y-1">
                            <label className="text-xs text-muted-foreground">Port</label>
                            <Input
                              type="number"
                              placeholder="e.g. 5432"
                              value={rulePort}
                              onChange={(e) => setRulePort(e.target.value)}
                              className="w-28"
                            />
                          </div>
                          <Button
                            size="sm"
                            onClick={() => handleAddRule(peering)}
                            disabled={actionLoading === `rule-add-${peering.id}` || !rulePort}
                          >
                            {actionLoading === `rule-add-${peering.id}` ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              "Add Rule"
                            )}
                          </Button>
                          <Button variant="ghost" size="sm" onClick={() => { setShowAddRule(null); setRulePort(""); }}>
                            Cancel
                          </Button>
                        </div>
                      ) : (
                        <Button
                          variant="outline"
                          size="sm"
                          className="text-xs"
                          onClick={() => { setShowAddRule(peering.id); setRulePort(""); setRuleDirection("a_to_b"); }}
                        >
                          <Plus className="h-3 w-3 mr-1" /> Add Rule
                        </Button>
                      )}
                    </CardContent>
                  )}
                </Card>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
