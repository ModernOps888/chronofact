import React, { useState, useEffect } from 'react';
import { CostMetricsSummary, ToolRoutingResult } from '../types';
import { Coins, Zap, Scissors, Database, RefreshCw, ShieldCheck, Cpu } from 'lucide-react';

interface Props {
  onRouteTools?: (query: string, topK: number) => Promise<ToolRoutingResult | null>;
}

export const CostOptimizerHUD: React.FC<Props> = () => {
  const [metrics, setMetrics] = useState<CostMetricsSummary | null>(null);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState('Search the web for the latest Rust 1.89 async stabilization notes');
  const [topK, setTopK] = useState(3);
  const [routeResult, setRouteResult] = useState<ToolRoutingResult | null>(null);
  const [routingInProgress, setRoutingInProgress] = useState(false);

  useEffect(() => {
    fetchMetrics();
    const interval = setInterval(fetchMetrics, 3000);
    return () => clearInterval(interval);
  }, []);

  const fetchMetrics = async () => {
    try {
      const res = await fetch('/api/cost/metrics');
      if (res.ok) {
        const data = await res.json();
        setMetrics(data);
      }
    } catch (e) {
      console.error('Failed to fetch cost metrics:', e);
    }
  };

  const handleSimulateRouting = async () => {
    if (!query.trim()) return;
    setRoutingInProgress(true);
    try {
      const res = await fetch('/api/cost/route', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query, top_k: topK }),
      });
      if (res.ok) {
        const data: ToolRoutingResult = await res.json();
        setRouteResult(data);
        await fetchMetrics();
      }
    } catch (e) {
      console.error('Routing simulation failed:', e);
    } finally {
      setRoutingInProgress(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Pillar 4 Header */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/30 rounded-2xl p-6 backdrop-blur-xl shadow-2xl gold-border-glow">
        <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 border-b border-slate-800 pb-6 mb-6">
          <div>
            <div className="flex items-center gap-2 text-amber-400 font-mono text-xs uppercase tracking-wider mb-1 font-bold">
              <Zap className="w-4 h-4 text-amber-400" /> Pillar 4: Token Context Bloat & Cost Bleed Prevention
            </div>
            <h2 className="text-2xl font-black text-white tracking-tight font-mono">
              TF-IDF Tool Router & Prompt Cache Prefix Engine
            </h2>
            <p className="text-slate-400 text-sm mt-1">
              Prunes unneeded tool schemas with cosine similarity routing, enforces deterministic prompt cache prefixes, and caches idempotent tool results.
            </p>
          </div>

          <button
            onClick={() => {
              setLoading(true);
              fetchMetrics().finally(() => setLoading(false));
            }}
            disabled={loading}
            className="flex items-center gap-2 px-4 py-2 bg-[#121827] border border-amber-500/30 rounded-xl text-xs font-mono font-bold text-amber-300 hover:bg-[#1a233a] transition-all shadow-md"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} /> Refresh Telemetry
          </button>
        </div>

        {/* Real-time KPI Metric Cards */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div className="bg-[#101626] border border-amber-500/20 p-4 rounded-xl">
            <div className="flex items-center justify-between text-slate-400 text-xs font-mono mb-1">
              <span>Tokens Saved</span>
              <Scissors className="w-4 h-4 text-emerald-400" />
            </div>
            <div className="text-2xl font-mono font-black text-emerald-400">
              {metrics ? metrics.total_tokens_saved.toLocaleString() : '0'}
            </div>
            <div className="text-[11px] font-mono text-slate-400 mt-1">
              Avg ~{metrics ? Math.round(metrics.average_tokens_saved_per_turn) : 0} tokens/turn
            </div>
          </div>

          <div className="bg-[#101626] border border-amber-500/20 p-4 rounded-xl">
            <div className="flex items-center justify-between text-slate-400 text-xs font-mono mb-1">
              <span>Cost Saved (USD)</span>
              <Coins className="w-4 h-4 text-amber-400" />
            </div>
            <div className="text-2xl font-mono font-black text-amber-300">
              ${metrics ? metrics.estimated_usd_saved.toFixed(4) : '0.0000'}
            </div>
            <div className="text-[11px] font-mono text-slate-400 mt-1">
              @ $3.00/1M token benchmark
            </div>
          </div>

          <div className="bg-[#101626] border border-amber-500/20 p-4 rounded-xl">
            <div className="flex items-center justify-between text-slate-400 text-xs font-mono mb-1">
              <span>Tools Pruned</span>
              <Cpu className="w-4 h-4 text-cyan-400" />
            </div>
            <div className="text-2xl font-mono font-black text-cyan-300">
              {metrics ? metrics.total_tools_pruned.toLocaleString() : '0'}
            </div>
            <div className="text-[11px] font-mono text-slate-400 mt-1">
              Across {metrics ? metrics.total_queries_optimized : 0} queries
            </div>
          </div>

          <div className="bg-[#101626] border border-amber-500/20 p-4 rounded-xl">
            <div className="flex items-center justify-between text-slate-400 text-xs font-mono mb-1">
              <span>Tool Cache Hit Rate</span>
              <Database className="w-4 h-4 text-purple-400" />
            </div>
            <div className="text-2xl font-mono font-black text-purple-300">
              {metrics ? `${metrics.cache_hit_rate_pct.toFixed(1)}%` : '0.0%'}
            </div>
            <div className="text-[11px] font-mono text-slate-400 mt-1">
              {metrics ? metrics.active_cache_entries : 0} active cached results
            </div>
          </div>
        </div>
      </div>

      {/* Interactive Tool Routing & Schema Pruning Simulator */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/30 rounded-2xl p-6 backdrop-blur-xl shadow-2xl gold-border-glow">
        <div className="flex items-center gap-2 text-amber-400 font-mono text-xs uppercase tracking-wider mb-2 font-bold">
          <Scissors className="w-4 h-4 text-amber-400" /> Interactive TF-IDF Schema Pruner
        </div>
        <h3 className="text-xl font-bold text-white font-mono mb-4">
          Test Query Against Antigravity & ChronoFact MCP Schemas
        </h3>

        <div className="space-y-4">
          <div className="flex flex-col md:flex-row gap-3">
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Enter agent query to evaluate which tools are dynamically injected..."
              className="flex-1 bg-[#121827] border border-slate-700 focus:border-amber-400 rounded-xl px-4 py-3 text-sm font-mono text-slate-200 focus:outline-none focus:ring-1 focus:ring-amber-400"
            />
            <div className="flex items-center gap-2">
              <span className="text-xs font-mono text-slate-400">Top-K:</span>
              <select
                value={topK}
                onChange={(e) => setTopK(Number(e.target.value))}
                className="bg-[#121827] border border-slate-700 rounded-xl px-3 py-3 text-sm font-mono text-amber-300 focus:outline-none cursor-pointer"
              >
                <option value={2}>2 Tools</option>
                <option value={3}>3 Tools</option>
                <option value={4}>4 Tools</option>
                <option value={5}>5 Tools</option>
              </select>
              <button
                onClick={handleSimulateRouting}
                disabled={routingInProgress}
                className="px-5 py-3 bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 hover:from-amber-300 hover:to-amber-400 text-black font-black text-sm font-mono rounded-xl transition-all shadow-gold-glow flex items-center gap-2 whitespace-nowrap"
              >
                {routingInProgress ? (
                  <RefreshCw className="w-4 h-4 animate-spin" />
                ) : (
                  <Zap className="w-4 h-4" />
                )}
                Route & Prune
              </button>
            </div>
          </div>

          {/* Quick preset queries */}
          <div className="flex flex-wrap gap-2 text-xs font-mono">
            <span className="text-slate-500 py-1">Try presets:</span>
            {[
              'Check model knowledge cutoff for Claude 3.5 Sonnet',
              'Search web for Rust 2026 tokio async features',
              'Verify these atomic statements against ground truth sources',
              'Execute powershell command to list active directories',
              'Save architectural invariant into project truth dossier',
            ].map((preset, idx) => (
              <button
                key={idx}
                onClick={() => setQuery(preset)}
                className="px-2.5 py-1 bg-[#101626] hover:bg-[#1a233a] text-slate-300 hover:text-amber-300 rounded-lg border border-slate-800 transition-all"
              >
                {preset.slice(0, 38)}...
              </button>
            ))}
          </div>
        </div>

        {/* Live Simulation Output */}
        {routeResult && (
          <div className="mt-6 pt-6 border-t border-slate-800/80 space-y-5">
            {/* Efficiency Banner */}
            <div className="bg-[#101626] border border-emerald-500/40 rounded-xl p-4 flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-emerald-500/20 border border-emerald-500/40 flex items-center justify-center text-emerald-400 font-bold">
                  {routeResult.cost_reduction_pct.toFixed(0)}%
                </div>
                <div>
                  <div className="text-sm font-bold text-white font-mono">
                    Context Token Reduction Achieved
                  </div>
                  <div className="text-xs text-slate-400 font-mono">
                    Pruned <span className="text-emerald-400 font-bold">{routeResult.pruned_tools}</span> irrelevant tools • Saved ~<span className="text-emerald-400 font-bold">{routeResult.tokens_saved}</span> prompt tokens this turn
                  </div>
                </div>
              </div>

              <div className="flex items-center gap-2 text-xs font-mono">
                <span className="px-3 py-1.5 bg-emerald-500/10 text-emerald-300 border border-emerald-500/30 rounded-lg font-bold">
                  Zero Context Bloat Enforced
                </span>
              </div>
            </div>

            {/* Selected vs Pruned Breakdown */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {/* Injected Tools */}
              <div className="bg-[#0e1422] border border-amber-500/30 rounded-xl p-4">
                <div className="flex items-center justify-between text-xs font-mono font-bold text-amber-400 mb-3">
                  <span className="flex items-center gap-1.5">
                    <Zap className="w-3.5 h-3.5 text-amber-400" />
                    Injected Tools ({routeResult.selected_tools.length})
                  </span>
                  <span className="text-slate-400 text-[10px]">Ranked by TF-IDF Cosine</span>
                </div>
                <div className="space-y-2.5">
                  {routeResult.selected_tools.map((tool, idx) => (
                    <div
                      key={idx}
                      className="bg-[#121827] border border-amber-500/20 rounded-lg p-3 text-xs font-mono"
                    >
                      <div className="flex items-center justify-between mb-1">
                        <span className="text-amber-300 font-bold">{tool.name}</span>
                        <span className="px-1.5 py-0.5 bg-amber-500/20 text-amber-200 text-[10px] rounded">
                          {tool.category}
                        </span>
                      </div>
                      <p className="text-slate-400 text-[11px] line-clamp-2">{tool.description}</p>
                      <div className="mt-2 text-[10px] text-slate-500 flex items-center gap-1">
                        <span className="text-slate-400">Params:</span> {tool.parameters.join(', ')}
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              {/* Cache Prefix Anchor & Pipeline Specs */}
              <div className="bg-[#0e1422] border border-slate-800 rounded-xl p-4 space-y-4">
                <div className="flex items-center justify-between text-xs font-mono font-bold text-slate-300">
                  <span className="flex items-center gap-1.5">
                    <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
                    Prompt Cache Prefix Anchor
                  </span>
                  <span className="text-emerald-400 text-[10px]">Deterministic V1</span>
                </div>

                <div className="bg-[#080c14] border border-slate-800 rounded-lg p-3 font-mono text-xs text-slate-300 break-all select-all">
                  {routeResult.prompt_cache_anchor}
                </div>

                <div className="text-xs font-mono text-slate-400 space-y-2">
                  <div className="flex items-center gap-2">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                    <span>Anthropic Prompt Caching: <strong className="text-white">5-minute rolling TTL supported</strong></span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                    <span>OpenAI Prompt Caching: <strong className="text-white">1024-token prefix aligned</strong></span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                    <span>Idempotent Cache: <strong className="text-white">SHA-256 tool+arg hashing</strong></span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Deep Architecture Explainer Card */}
      <div className="bg-[#0b0f19]/95 border border-slate-800 rounded-2xl p-6 font-mono text-xs text-slate-300">
        <h4 className="text-sm font-bold text-amber-300 mb-3 flex items-center gap-2">
          <Database className="w-4 h-4 text-amber-400" />
          The 3-Layer Cost Elimination Pipeline (Proven in mcplex)
        </h4>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-slate-400">
          <div className="bg-[#101626] p-3.5 rounded-xl border border-slate-800/80">
            <div className="text-amber-400 font-bold mb-1">1. Static Prefix Alignment</div>
            <p className="text-[11px] leading-relaxed">
              Positions system instructions, immutable project invariants, and stable schemas at the exact prefix of prompts, hitting provider caches for 50-80% discounts.
            </p>
          </div>
          <div className="bg-[#101626] p-3.5 rounded-xl border border-slate-800/80">
            <div className="text-cyan-400 font-bold mb-1">2. TF-IDF Schema Pruning</div>
            <p className="text-[11px] leading-relaxed">
              Extracts unigrams from the incoming query and computes cosine similarity against tool signatures. Only top-K relevant schemas enter context, dropping prompt size by 70-90%.
            </p>
          </div>
          <div className="bg-[#101626] p-3.5 rounded-xl border border-slate-800/80">
            <div className="text-purple-400 font-bold mb-1">3. Idempotent Tool Cache</div>
            <p className="text-[11px] leading-relaxed">
              Deterministic tool executions (e.g. repeated file reads, doc queries, stable web searches) are hashed via SHA-256 and served in 0ms with zero outbound token consumption.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
