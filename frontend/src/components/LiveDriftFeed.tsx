import React, { useState, useEffect, useMemo } from 'react';
import { DriftEvent } from '../types';
import {
  AlertTriangle,
  Clock,
  RefreshCw,
  Zap,
  Bot,
  ShieldAlert,
  CheckCircle2,
  Terminal,
  Search,
  Monitor,
  Cpu,
  Globe,
  ChevronDown,
  ChevronUp,
  Copy,
  Check,
  Code2
} from 'lucide-react';

interface Props {
  events: DriftEvent[];
  onRefresh: () => Promise<void>;
}

export const LiveDriftFeed: React.FC<Props> = ({ events, onRefresh }) => {
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [activeFilter, setActiveFilter] = useState<'all' | 'ide' | 'contradictions' | 'grounding'>('all');
  const [expandedEventId, setExpandedEventId] = useState<number | null>(null);
  const [copiedId, setCopiedId] = useState<number | null>(null);

  useEffect(() => {
    if (!autoRefresh) return;
    const interval = setInterval(() => {
      onRefresh();
    }, 2000);
    return () => clearInterval(interval);
  }, [autoRefresh, onRefresh]);

  const handleManualRefresh = async () => {
    setIsRefreshing(true);
    await onRefresh();
    setIsRefreshing(false);
  };

  const handleCopy = (id: number, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1800);
  };

  const filteredEvents = useMemo(() => {
    return events.filter((e) => {
      // Category filter
      if (activeFilter === 'ide' && !e.project_id.toLowerCase().includes('ide')) return false;
      if (activeFilter === 'contradictions' && !e.intervention_type.includes('CONTRADICTION')) return false;
      if (activeFilter === 'grounding' && !e.intervention_type.includes('GROUNDING')) return false;

      // Text query
      if (!searchQuery.trim()) return true;
      const q = searchQuery.toLowerCase();
      return (
        e.query.toLowerCase().includes(q) ||
        e.model_id.toLowerCase().includes(q) ||
        e.outdated_topics_caught.toLowerCase().includes(q) ||
        e.ground_truth_retrieved.toLowerCase().includes(q) ||
        e.project_id.toLowerCase().includes(q)
      );
    });
  }, [events, activeFilter, searchQuery]);

  const ideInterceptions = events.filter((e) => e.project_id.toLowerCase().includes('ide'));
  const contradictions = events.filter((e) => e.intervention_type.includes('CONTRADICTION'));
  const webGroundings = events.filter((e) => e.intervention_type.includes('GROUNDING'));

  const getVendorBadge = (modelId: string) => {
    const m = modelId.toLowerCase();
    if (m.includes('claude') || m.includes('opus') || m.includes('sonnet')) {
      return { vendor: 'Anthropic', color: 'bg-amber-500/20 text-amber-300 border-amber-500/40' };
    }
    if (m.includes('gpt') || m.includes('astra') || m.includes('sol') || m.includes('o3') || m.includes('o1')) {
      return { vendor: 'OpenAI', color: 'bg-emerald-500/20 text-emerald-300 border-emerald-500/40' };
    }
    if (m.includes('deepseek')) {
      return { vendor: 'DeepSeek', color: 'bg-cyan-500/20 text-cyan-300 border-cyan-500/40' };
    }
    if (m.includes('grok')) {
      return { vendor: 'xAI', color: 'bg-purple-500/20 text-purple-300 border-purple-500/40' };
    }
    if (m.includes('gemini')) {
      return { vendor: 'Google', color: 'bg-blue-500/20 text-blue-300 border-blue-500/40' };
    }
    return { vendor: 'ChronoFact Engine', color: 'bg-slate-700/40 text-slate-300 border-slate-600' };
  };

  return (
    <div className="space-y-6">
      {/* Header Banner */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/30 rounded-2xl p-6 backdrop-blur-xl shadow-2xl gold-border-glow">
        <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 border-b border-slate-800 pb-5 mb-6">
          <div>
            <div className="flex items-center gap-2 text-amber-400 font-mono text-xs uppercase tracking-wider mb-1 font-bold">
              <Zap className="w-4 h-4 text-amber-400 animate-pulse" /> Live Interceptor & Outdated Belief Radar
            </div>
            <h2 className="text-2xl font-black text-white tracking-tight font-mono">
              Real-Time IDE Interception Ledger
            </h2>
            <p className="text-slate-400 text-sm mt-1 max-w-3xl">
              Live telemetry tracking every prompt, model evaluation, and knowledge horizon drift originating from your 
              <span className="text-amber-300 font-bold"> Google Antigravity IDE</span> and connected MCP clients.
            </p>
          </div>

          <div className="flex items-center gap-3">
            <button
              onClick={() => setAutoRefresh(!autoRefresh)}
              className={`px-3.5 py-2 rounded-xl text-xs font-mono font-bold transition border ${
                autoRefresh
                  ? 'bg-emerald-500/15 text-emerald-300 border-emerald-500/40 shadow-[0_0_10px_rgba(16,185,129,0.2)]'
                  : 'bg-[#101422] text-slate-500 border-slate-800'
              }`}
            >
              {autoRefresh ? '● Streaming Live (2.0s)' : '○ Stream Paused'}
            </button>
            <button
              onClick={handleManualRefresh}
              disabled={isRefreshing}
              className="bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 hover:from-amber-300 hover:to-amber-400 text-black px-4 py-2 rounded-xl text-xs font-black flex items-center gap-1.5 transition shadow-gold-glow cursor-pointer"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isRefreshing ? 'animate-spin' : ''}`} />
              Refresh Feed
            </button>
          </div>
        </div>

        {/* Live Counter Cards in Imperial Gold */}
        <div className="grid grid-cols-1 sm:grid-cols-4 gap-4">
          <div className="bg-[#0f1422] border border-amber-500/25 p-4 rounded-xl shadow-inner">
            <div className="text-xs font-mono text-amber-400 uppercase font-bold flex items-center justify-between">
              Total Interceptions
              <Zap className="w-3.5 h-3.5 text-amber-400" />
            </div>
            <div className="text-3xl font-black text-white mt-1 font-mono">{events.length}</div>
            <div className="text-xs text-slate-400 mt-1 font-mono">100% prompt coverage</div>
          </div>

          <div className="bg-[#0f1422] border border-amber-500/25 p-4 rounded-xl shadow-inner">
            <div className="text-xs font-mono text-amber-400 uppercase font-bold flex items-center justify-between">
              Antigravity IDE Tracks
              <Terminal className="w-3.5 h-3.5 text-amber-400" />
            </div>
            <div className="text-3xl font-black text-amber-300 mt-1 font-mono">
              {ideInterceptions.length}
            </div>
            <div className="text-xs text-slate-400 mt-1 font-mono">Native desktop MCP events</div>
          </div>

          <div className="bg-[#0f1422] border border-amber-500/25 p-4 rounded-xl shadow-inner">
            <div className="text-xs font-mono text-amber-400 uppercase font-bold flex items-center justify-between">
              Contradictions Blocked
              <ShieldAlert className="w-3.5 h-3.5 text-amber-400" />
            </div>
            <div className="text-3xl font-black text-rose-400 mt-1 font-mono">
              {contradictions.length}
            </div>
            <div className="text-xs text-slate-400 mt-1 font-mono">Hallucinations neutralized</div>
          </div>

          <div className="bg-[#0f1422] border border-emerald-500/30 p-4 rounded-xl shadow-inner">
            <div className="text-xs font-mono text-emerald-400 uppercase font-bold flex items-center justify-between">
              Web Grounding Hits
              <Globe className="w-3.5 h-3.5 text-emerald-400" />
            </div>
            <div className="text-3xl font-black text-emerald-300 mt-1 font-mono">{webGroundings.length}</div>
            <div className="text-xs text-slate-400 mt-1 font-mono">Sanitized 2026 web sources</div>
          </div>
        </div>
      </div>

      {/* Filter and Search Bar */}
      <div className="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-3 bg-[#0a0e1a]/90 border border-slate-800 p-3 rounded-xl backdrop-blur-md">
        <div className="flex items-center gap-2 overflow-x-auto pb-1 sm:pb-0">
          <button
            onClick={() => setActiveFilter('all')}
            className={`px-3 py-1.5 rounded-lg text-xs font-mono font-bold transition whitespace-nowrap ${
              activeFilter === 'all'
                ? 'bg-amber-400 text-black shadow-gold-glow'
                : 'bg-[#121727] text-slate-400 hover:text-white border border-slate-800'
            }`}
          >
            All Events ({events.length})
          </button>
          <button
            onClick={() => setActiveFilter('ide')}
            className={`px-3 py-1.5 rounded-lg text-xs font-mono font-bold transition flex items-center gap-1.5 whitespace-nowrap ${
              activeFilter === 'ide'
                ? 'bg-amber-400 text-black shadow-gold-glow'
                : 'bg-[#121727] text-slate-400 hover:text-white border border-slate-800'
            }`}
          >
            <Terminal className="w-3 h-3" /> Antigravity IDE ({ideInterceptions.length})
          </button>
          <button
            onClick={() => setActiveFilter('contradictions')}
            className={`px-3 py-1.5 rounded-lg text-xs font-mono font-bold transition flex items-center gap-1.5 whitespace-nowrap ${
              activeFilter === 'contradictions'
                ? 'bg-rose-500 text-white shadow-rose-950/40'
                : 'bg-[#121727] text-slate-400 hover:text-white border border-slate-800'
            }`}
          >
            <ShieldAlert className="w-3 h-3" /> Contradictions ({contradictions.length})
          </button>
          <button
            onClick={() => setActiveFilter('grounding')}
            className={`px-3 py-1.5 rounded-lg text-xs font-mono font-bold transition flex items-center gap-1.5 whitespace-nowrap ${
              activeFilter === 'grounding'
                ? 'bg-emerald-500 text-black shadow-emerald-950/40'
                : 'bg-[#121727] text-slate-400 hover:text-white border border-slate-800'
            }`}
          >
            <Globe className="w-3 h-3" /> Grounding ({webGroundings.length})
          </button>
        </div>

        <div className="relative min-w-[260px]">
          <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
          <input
            type="text"
            placeholder="Search prompts, models, crates, entities..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full bg-[#070b14] border border-slate-800 rounded-lg pl-9 pr-3 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-amber-400 font-mono"
          />
        </div>
      </div>

      {/* Live Interception Stream Feed */}
      <div className="space-y-4">
        <div className="flex items-center justify-between px-1">
          <h3 className="text-sm font-bold font-mono text-amber-300 uppercase tracking-wider flex items-center gap-2">
            <Clock className="w-4 h-4 text-amber-400" /> Interception Ledger Timeline
          </h3>
          <span className="text-xs text-slate-400 font-mono">
            Displaying {filteredEvents.length} of {events.length} verified events
          </span>
        </div>

        {filteredEvents.length === 0 ? (
          <div className="bg-[#0c101c] border border-dashed border-amber-500/30 p-12 text-center rounded-2xl">
            <Bot className="w-12 h-12 text-amber-400/40 mx-auto mb-3" />
            <div className="text-white font-bold font-mono">No matching drift events found.</div>
            <div className="text-slate-400 text-sm mt-1">
              Ask a question about a recent 2026 tech tool, or run a check in Antigravity IDE to watch real-time interceptions stream here.
            </div>
          </div>
        ) : (
          filteredEvents.map((event) => {
            const isIde = event.project_id.toLowerCase().includes('ide');
            const isContradiction = event.intervention_type.includes('CONTRADICTION');
            const isGrounding = event.intervention_type.includes('GROUNDING');
            const isExpanded = expandedEventId === event.id;
            const vendorInfo = getVendorBadge(event.model_id);

            return (
              <div
                key={event.id}
                className={`bg-[#0c111e]/95 border rounded-2xl p-5 shadow-xl transition-all duration-300 ${
                  isContradiction
                    ? 'border-rose-500/40 shadow-rose-950/20'
                    : isIde
                    ? 'border-amber-400/40 shadow-gold-glow/20'
                    : 'border-slate-800 hover:border-amber-500/30'
                }`}
              >
                {/* Card Top Metadata Bar */}
                <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-800/80 pb-3 mb-3">
                  <div className="flex items-center gap-2 flex-wrap">
                    {/* Explicit IDE Origin Tag */}
                    {isIde ? (
                      <span className="px-2.5 py-0.5 rounded-full text-[11px] font-mono font-bold bg-amber-400/20 text-amber-300 border border-amber-400/50 flex items-center gap-1 shadow-sm">
                        <Monitor className="w-3 h-3 text-amber-400" /> GOOGLE ANTIGRAVITY IDE
                      </span>
                    ) : (
                      <span className="px-2.5 py-0.5 rounded-full text-[11px] font-mono font-bold bg-slate-800 text-slate-300 border border-slate-700 flex items-center gap-1">
                        <Cpu className="w-3 h-3 text-slate-400" /> BACKEND API SESSION
                      </span>
                    )}

                    {/* Intervention Type Pill */}
                    <span
                      className={`px-2.5 py-0.5 rounded-full text-[11px] font-mono font-bold ${
                        isContradiction
                          ? 'bg-rose-500/20 text-rose-300 border border-rose-500/40'
                          : isGrounding
                          ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40'
                          : 'bg-amber-500/20 text-amber-300 border border-amber-500/40'
                      }`}
                    >
                      {event.intervention_type}
                    </span>

                    {/* Model & Vendor Badge */}
                    <span className={`px-2 py-0.5 rounded text-[11px] font-mono font-bold border flex items-center gap-1 ${vendorInfo.color}`}>
                      <Code2 className="w-3 h-3" />
                      {vendorInfo.vendor} • {event.model_id}
                    </span>

                    {/* Pre-Release Freeze Lag Badge */}
                    {event.days_post_freeze > 0 && (
                      <span className="text-[11px] font-mono text-amber-400 bg-amber-950/40 border border-amber-500/30 px-2 py-0.5 rounded font-bold">
                        +{event.days_post_freeze}d Freeze Lag
                      </span>
                    )}
                  </div>

                  <div className="flex items-center gap-3">
                    <span className="text-xs font-mono text-slate-400 flex items-center gap-1.5">
                      <Clock className="w-3.5 h-3.5 text-slate-500" />
                      {new Date(event.timestamp).toLocaleTimeString()} ({new Date(event.timestamp).toLocaleDateString()})
                    </span>

                    <button
                      onClick={() => setExpandedEventId(isExpanded ? null : event.id)}
                      className="text-slate-400 hover:text-amber-300 p-1 rounded transition text-xs font-mono flex items-center gap-1 cursor-pointer"
                      title="Toggle detailed telemetry view"
                    >
                      {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                    </button>
                  </div>
                </div>

                {/* Primary Content Grid */}
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  {/* Left: What Was Caught */}
                  <div className="bg-[#080b12] border border-slate-800/80 p-3.5 rounded-xl">
                    <div className="text-[11px] font-mono text-amber-400 font-bold uppercase tracking-wide mb-1 flex items-center justify-between">
                      <span className="flex items-center gap-1.5">
                        <AlertTriangle className="w-3.5 h-3.5 text-amber-400" />
                        Prompt / Claim Intercepted:
                      </span>
                      <button
                        onClick={() => handleCopy(event.id, event.query)}
                        className="text-slate-500 hover:text-white transition"
                        title="Copy prompt"
                      >
                        {copiedId === event.id ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
                      </button>
                    </div>
                    <div className="text-xs text-slate-200 font-mono bg-[#0d121c] p-2.5 rounded border border-slate-800/60 break-words font-semibold leading-relaxed">
                      "{event.query}"
                    </div>
                    <div className="text-[11px] text-slate-400 mt-2 font-mono flex items-center justify-between">
                      <span>Detected Clashes: <strong className="text-amber-300">{event.outdated_topics_caught}</strong></span>
                    </div>
                  </div>

                  {/* Right: How ChronoFact Corrected It */}
                  <div className={`bg-[#080b12] p-3.5 rounded-xl border ${
                    isContradiction ? 'border-rose-900/40' : 'border-emerald-900/40'
                  }`}>
                    <div className={`text-[11px] font-mono font-bold uppercase tracking-wide mb-1 flex items-center gap-1.5 ${
                      isContradiction ? 'text-rose-400' : 'text-emerald-400'
                    }`}>
                      {isContradiction ? (
                        <>
                          <ShieldAlert className="w-3.5 h-3.5 text-rose-400" />
                          Contradiction Neutralized:
                        </>
                      ) : (
                        <>
                          <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
                          2026 Ground Truth Injected:
                        </>
                      )}
                    </div>
                    <div className={`text-xs font-mono p-2.5 rounded border break-words font-semibold leading-relaxed ${
                      isContradiction
                        ? 'bg-[#170a0d] text-rose-200 border-rose-800/50'
                        : 'bg-[#0a1410] text-emerald-200 border-emerald-800/50'
                    }`}>
                      {event.ground_truth_retrieved}
                    </div>
                    <div className="text-[11px] text-slate-400 mt-2 font-mono flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <span>Risk Score:</span>
                        <div className="w-20 bg-slate-800 h-2 rounded-full overflow-hidden inline-flex">
                          <div
                            className={`h-full rounded-full ${
                              event.temporal_risk_score > 0.6
                                ? 'bg-rose-500'
                                : event.temporal_risk_score > 0.3
                                ? 'bg-amber-400'
                                : 'bg-emerald-400'
                            }`}
                            style={{ width: `${Math.max(5, event.temporal_risk_score * 100)}%` }}
                          />
                        </div>
                        <span className="font-bold text-white">{(event.temporal_risk_score * 100).toFixed(0)}%</span>
                      </div>
                      <span className="text-slate-500">Event #{event.id}</span>
                    </div>
                  </div>
                </div>

                {/* Expanded Telemetry Drawer */}
                {isExpanded && (
                  <div className="mt-4 pt-4 border-t border-slate-800/80 animate-in fade-in slide-in-from-top-2 duration-200">
                    <div className="bg-[#05080f] rounded-xl p-4 border border-slate-800 font-mono text-xs space-y-3">
                      <div className="text-amber-400 font-bold flex items-center justify-between border-b border-slate-800/60 pb-2">
                        <span>Detailed Telemetry Breakdown</span>
                        <span className="text-slate-500 text-[11px]">Integration: Stdio JSON-RPC 2.0 (MCP)</span>
                      </div>

                      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 text-[11px]">
                        <div>
                          <span className="text-slate-500 block">Client Origin:</span>
                          <span className="text-amber-300 font-bold">{isIde ? 'Google Antigravity IDE (Windows)' : 'Direct Web Client'}</span>
                        </div>
                        <div>
                          <span className="text-slate-500 block">Project Namespace:</span>
                          <span className="text-white font-bold">{event.project_id}</span>
                        </div>
                        <div>
                          <span className="text-slate-500 block">Temporal Lag Delta:</span>
                          <span className="text-white font-bold">{event.days_post_freeze} days past training freeze</span>
                        </div>
                      </div>

                      <div className="mt-2">
                        <span className="text-slate-500 block text-[11px] mb-1">Raw Event Audit Record:</span>
                        <pre className="bg-[#020408] p-2.5 rounded border border-slate-800 text-[11px] text-slate-300 overflow-x-auto">
                          {JSON.stringify(event, null, 2)}
                        </pre>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
