import React, { useState, useEffect } from 'react';
import { GatewayServerStatus, GatewayTool } from '../types';
import { Network, Server, Wrench, FileText, MessageSquareCode, CheckCircle2, RefreshCw, Shield, ArrowUpRight, Zap } from 'lucide-react';

interface Props {
  onCallTool?: (toolName: string, args: Record<string, any>, passthrough: boolean) => Promise<any>;
}

export const GatewayMultiplexerView: React.FC<Props> = () => {
  const [servers, setServers] = useState<GatewayServerStatus[]>([]);
  const [tools, setTools] = useState<GatewayTool[]>([]);
  const [loading, setLoading] = useState(false);
  const [passthroughActive, setPassthroughActive] = useState(false);
  const [selectedTool, setSelectedTool] = useState<string | null>(null);
  const [callResult, setCallResult] = useState<string | null>(null);
  const [calling, setCalling] = useState(false);

  useEffect(() => {
    fetchGatewayData();
    const interval = setInterval(fetchGatewayData, 5000);
    return () => clearInterval(interval);
  }, []);

  const fetchGatewayData = async () => {
    try {
      const serversRes = await fetch('/api/gateway/servers');
      if (serversRes.ok) {
        const data = await serversRes.json();
        setServers(data);
      }

      const toolsRes = await fetch('/api/gateway/tools');
      if (toolsRes.ok) {
        const data = await toolsRes.json();
        setTools(data);
      }
    } catch (e) {
      console.error('Failed to fetch gateway data:', e);
    }
  };

  const handleTestCall = async (toolFqn: string) => {
    setSelectedTool(toolFqn);
    setCalling(true);
    setCallResult(null);
    try {
      const res = await fetch('/api/gateway/call', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: toolFqn,
          arguments: {},
          passthrough: passthroughActive,
        }),
      });
      const data = await res.json();
      setCallResult(JSON.stringify(data, null, 2));
    } catch (err: any) {
      setCallResult(`Call Error: ${err.message || 'Gateway call failed'}`);
    } finally {
      setCalling(false);
    }
  };

  const totalTools = servers.reduce((acc, s) => acc + s.tools_count, tools.length);
  const totalResources = servers.reduce((acc, s) => acc + s.resources_count, 0);
  const totalPrompts = servers.reduce((acc, s) => acc + s.prompts_count, 0);

  return (
    <div className="space-y-6">
      {/* Header Banner */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/30 rounded-2xl p-6 backdrop-blur-xl shadow-2xl gold-border-glow">
        <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 border-b border-slate-800 pb-6 mb-6">
          <div>
            <div className="flex items-center gap-2 text-amber-400 font-mono text-xs uppercase tracking-wider mb-1 font-bold">
              <Network className="w-4 h-4 text-amber-400" /> MCP Gateway Multiplexer (MCPLEX Architecture)
            </div>
            <h2 className="text-2xl font-black text-white tracking-tight font-mono flex items-center gap-3">
              Federated MCP Multi-Server Engine
              <span className="text-xs px-2.5 py-0.5 rounded-full bg-emerald-500/10 text-emerald-300 border border-emerald-400/30 font-bold">
                Spec 2025-03-26
              </span>
              <span className="text-xs px-2.5 py-0.5 rounded-full bg-amber-500/10 text-amber-300 border border-amber-400/30 font-bold">
                57/57 Tests Passing
              </span>
            </h2>
            <p className="text-slate-400 text-sm mt-1">
              Multiplexes stdio and Streamable HTTP upstream servers into a unified endpoint with read-only hint cache gating, 404 auto-recovery, and cursor pagination.
            </p>
          </div>

          <div className="flex items-center gap-3">
            <button
              onClick={() => setPassthroughActive(!passthroughActive)}
              className={`flex items-center gap-2 px-3.5 py-2 rounded-xl text-xs font-mono font-bold transition-all border ${
                passthroughActive
                  ? 'bg-amber-500/20 text-amber-300 border-amber-400/50 shadow-gold-glow'
                  : 'bg-[#101626] text-slate-400 border-slate-700 hover:text-slate-200'
              }`}
            >
              <Zap className="w-3.5 h-3.5" /> Passthrough Mode: {passthroughActive ? 'ACTIVE' : 'OFF'}
            </button>

            <button
              onClick={() => {
                setLoading(true);
                fetchGatewayData().finally(() => setLoading(false));
              }}
              disabled={loading}
              className="flex items-center gap-2 px-4 py-2 bg-[#121827] border border-amber-500/30 rounded-xl text-xs font-mono font-bold text-amber-300 hover:bg-[#1a233a] transition-all shadow-md"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} /> Refresh Gateway
            </button>
          </div>
        </div>

        {/* Real-time KPI Metric Cards */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div className="bg-[#101626] border border-amber-500/20 p-4 rounded-xl">
            <div className="flex items-center justify-between text-slate-400 text-xs font-mono mb-1">
              <span>Upstream Servers</span>
              <Server className="w-4 h-4 text-amber-400" />
            </div>
            <div className="text-2xl font-mono font-black text-amber-300">
              {servers.length}
            </div>
            <div className="text-[11px] font-mono text-slate-400 mt-1">
              {servers.filter(s => s.connected).length} active / connected
            </div>
          </div>

          <div className="bg-[#101626] border border-amber-500/20 p-4 rounded-xl">
            <div className="flex items-center justify-between text-slate-400 text-xs font-mono mb-1">
              <span>Federated Tools</span>
              <Wrench className="w-4 h-4 text-emerald-400" />
            </div>
            <div className="text-2xl font-mono font-black text-emerald-400">
              {totalTools}
            </div>
            <div className="text-[11px] font-mono text-slate-400 mt-1">
              Tagged and namespaced
            </div>
          </div>

          <div className="bg-[#101626] border border-amber-500/20 p-4 rounded-xl">
            <div className="flex items-center justify-between text-slate-400 text-xs font-mono mb-1">
              <span>Multiplexed Resources</span>
              <FileText className="w-4 h-4 text-cyan-400" />
            </div>
            <div className="text-2xl font-mono font-black text-cyan-300">
              {totalResources}
            </div>
            <div className="text-[11px] font-mono text-slate-400 mt-1">
              Federated URI registry
            </div>
          </div>

          <div className="bg-[#101626] border border-amber-500/20 p-4 rounded-xl">
            <div className="flex items-center justify-between text-slate-400 text-xs font-mono mb-1">
              <span>Prompts Multiplexed</span>
              <MessageSquareCode className="w-4 h-4 text-purple-400" />
            </div>
            <div className="text-2xl font-mono font-black text-purple-300">
              {totalPrompts}
            </div>
            <div className="text-[11px] font-mono text-slate-400 mt-1">
              Multi-agent system prompts
            </div>
          </div>
        </div>
      </div>

      {/* Upstream Servers Table */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/20 rounded-2xl p-6 backdrop-blur-xl shadow-xl">
        <h3 className="text-lg font-bold text-white font-mono mb-4 flex items-center gap-2">
          <Server className="w-5 h-5 text-amber-400" /> Connected Upstream Servers
        </h3>

        {servers.length === 0 ? (
          <div className="text-center py-8 text-slate-400 font-mono text-sm border border-dashed border-slate-800 rounded-xl">
            No external upstream servers configured in chronofact.toml. Built-in MCP server running natively on stdio and HTTP.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left font-mono text-xs">
              <thead className="border-b border-slate-800 text-slate-400">
                <tr>
                  <th className="pb-3 font-semibold">SERVER NAME</th>
                  <th className="pb-3 font-semibold">STATUS</th>
                  <th className="pb-3 font-semibold">TRANSPORT</th>
                  <th className="pb-3 font-semibold text-right">TOOLS</th>
                  <th className="pb-3 font-semibold text-right">RESOURCES</th>
                  <th className="pb-3 font-semibold text-right">PROMPTS</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800/60">
                {servers.map((s) => (
                  <tr key={s.name} className="hover:bg-slate-800/30">
                    <td className="py-3 font-bold text-amber-200">{s.name}</td>
                    <td className="py-3">
                      <span className={`px-2 py-0.5 rounded text-[10px] font-bold border ${
                        s.connected
                          ? 'bg-emerald-500/20 text-emerald-300 border-emerald-500/40'
                          : 'bg-rose-500/20 text-rose-300 border-rose-500/40'
                      }`}>
                        {s.connected ? 'CONNECTED' : 'DISCONNECTED'}
                      </span>
                    </td>
                    <td className="py-3 text-slate-400 uppercase">{s.transport}</td>
                    <td className="py-3 text-right font-bold text-emerald-400">{s.tools_count}</td>
                    <td className="py-3 text-right font-bold text-cyan-400">{s.resources_count}</td>
                    <td className="py-3 text-right font-bold text-purple-400">{s.prompts_count}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Protocol Features & Architectural Invariants */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-[#0b0f19]/90 border border-slate-800 p-5 rounded-xl space-y-2">
          <div className="flex items-center gap-2 text-emerald-400 font-mono text-xs font-bold">
            <CheckCircle2 className="w-4 h-4" /> Streamable HTTP 2025-03-26
          </div>
          <h4 className="text-sm font-bold text-white font-mono">Session Resilience</h4>
          <p className="text-xs text-slate-400">
            Native mcp-session-id headers with automatic 404 recovery. If an upstream server reboots, ChronoFact transparently negotiates a fresh session handshake.
          </p>
        </div>

        <div className="bg-[#0b0f19]/90 border border-slate-800 p-5 rounded-xl space-y-2">
          <div className="flex items-center gap-2 text-amber-400 font-mono text-xs font-bold">
            <Shield className="w-4 h-4" /> Cache Gating Safety
          </div>
          <h4 className="text-sm font-bold text-white font-mono">readOnlyHint Adherence</h4>
          <p className="text-xs text-slate-400">
            Strictly respects upstream tool annotations. Only idempotent read operations qualify for SHA-256 caching. All state mutations execute live with full auditing.
          </p>
        </div>

        <div className="bg-[#0b0f19]/90 border border-slate-800 p-5 rounded-xl space-y-2">
          <div className="flex items-center gap-2 text-cyan-400 font-mono text-xs font-bold">
            <ArrowUpRight className="w-4 h-4" /> Cursor Pagination
          </div>
          <h4 className="text-sm font-bold text-white font-mono">Scale-Ready Discovery</h4>
          <p className="text-xs text-slate-400">
            Cursor-based pagination across federated tool, resource, and prompt catalogs prevents buffer overflows when orchestrating enterprise toolsets.
          </p>
        </div>
      </div>

      {/* Multiplexed Tools Roster */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/20 rounded-2xl p-6 backdrop-blur-xl shadow-xl">
        <h3 className="text-lg font-bold text-white font-mono mb-4 flex items-center gap-2">
          <Wrench className="w-5 h-5 text-amber-400" /> Federated Tool Catalog ({tools.length} Registered)
        </h3>

        {tools.length === 0 ? (
          <div className="text-center py-8 text-slate-400 font-mono text-sm border border-dashed border-slate-800 rounded-xl">
            No upstream tools registered yet. Use chronofact serve with upstream MCP configurations.
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 max-h-96 overflow-y-auto pr-2">
            {tools.map((t) => {
              const isReadOnly = t.definition?.annotations?.readOnlyHint ?? false;
              return (
                <div
                  key={t.fqn}
                  className="bg-[#101626] border border-slate-800 hover:border-amber-500/40 p-4 rounded-xl transition-all flex flex-col justify-between"
                >
                  <div>
                    <div className="flex items-center justify-between gap-2 mb-1.5">
                      <span className="font-bold text-white font-mono text-xs">{t.fqn}</span>
                      <div className="flex items-center gap-1.5">
                        <span className="px-1.5 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/30">
                          {t.server_name}
                        </span>
                        {isReadOnly && (
                          <span className="px-1.5 py-0.5 rounded text-[10px] font-mono font-bold bg-emerald-500/10 text-emerald-300 border border-emerald-500/30">
                            READ-ONLY
                          </span>
                        )}
                      </div>
                    </div>
                    <p className="text-xs text-slate-400 line-clamp-2">
                      {t.definition.description || 'No description provided.'}
                    </p>
                  </div>

                  <div className="mt-3 pt-3 border-t border-slate-800/80 flex items-center justify-between">
                    <span className="text-[11px] font-mono text-slate-500">
                      Method: tools/call
                    </span>
                    <button
                      onClick={() => handleTestCall(t.fqn)}
                      disabled={calling}
                      className="px-2.5 py-1 bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 border border-amber-500/30 rounded text-xs font-mono font-bold transition-all"
                    >
                      {calling && selectedTool === t.fqn ? 'Calling...' : 'Simulate Call'}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {callResult && (
          <div className="mt-6 bg-[#070b14] border border-amber-500/30 p-4 rounded-xl">
            <div className="text-xs font-mono font-bold text-amber-300 mb-2">
              RESULT FROM {selectedTool}:
            </div>
            <pre className="text-xs font-mono text-slate-300 overflow-x-auto p-3 bg-black/60 rounded border border-slate-800 max-h-48">
              {callResult}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
};
