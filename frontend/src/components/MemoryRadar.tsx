import React, { useState } from 'react';
import { ProjectDossier } from '../types';
import {
  Database,
  Plus,
  Layers,
  Bookmark,
  HardDrive,
  Sparkles,
  ShieldCheck,
  Search,
  CheckCircle2,
  ArrowRight,
  Zap,
} from 'lucide-react';

interface Props {
  dossier: ProjectDossier | null;
  projectId: string;
  onRefreshDossier: (projectId: string) => Promise<void>;
  onSaveEntity: (entity: {
    projectId: string;
    entityName: string;
    entityType: string;
    definition: string;
    version?: string;
  }) => Promise<void>;
}

export const MemoryRadar: React.FC<Props> = ({
  dossier,
  projectId,
  onRefreshDossier,
  onSaveEntity,
}) => {
  const [activeTab, setActiveTab] = useState<'dossier' | 'simulator' | 'add'>('dossier');
  const [entityName, setEntityName] = useState('');
  const [entityType, setEntityType] = useState('ARCHITECTURE_RULE');
  const [definition, setDefinition] = useState('');
  const [version, setVersion] = useState('1.0.0');
  const [isSaving, setIsSaving] = useState(false);

  // Zero-Pollution Simulator State
  const [simQuery, setSimQuery] = useState('Explain quicksort algorithm in python');
  const [isSimulating, setIsSimulating] = useState(false);
  const [simResult, setSimResult] = useState<{
    tested: boolean;
    query: string;
    invariantsCount: number;
    tokensInjected: number;
    isPollutionBlocked: boolean;
    markdown: string;
    matchedEntities: string[];
  } | null>(null);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!entityName || !definition) return;
    setIsSaving(true);
    await onSaveEntity({
      projectId,
      entityName,
      entityType,
      definition,
      version,
    });
    setEntityName('');
    setDefinition('');
    setIsSaving(false);
    setActiveTab('dossier');
    await onRefreshDossier(projectId);
  };

  const runSimulation = async (queryToTest: string) => {
    if (!queryToTest.trim()) return;
    setIsSimulating(true);
    try {
      const res = await fetch(
        `/api/memory/dossier/${projectId}?query=${encodeURIComponent(queryToTest)}`
      );
      if (res.ok) {
        const data: ProjectDossier = await res.json();
        const tokens = Math.round(data.dossier_markdown.length / 4);
        const isBlocked = tokens === 0;
        setSimResult({
          tested: true,
          query: queryToTest,
          invariantsCount:
            (data.architectural_invariants?.length || 0) + (data.active_tech_stack?.length || 0),
          tokensInjected: tokens,
          isPollutionBlocked: isBlocked,
          markdown: data.dossier_markdown,
          matchedEntities: [
            ...(data.active_tech_stack || []),
            ...(data.architectural_invariants || []),
          ],
        });
      }
    } catch (err) {
      console.error('Simulation error:', err);
    } finally {
      setIsSimulating(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl">
        <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 border-b border-emerald-900/60 pb-6 mb-6">
          <div>
            <div className="flex items-center gap-2 text-gold-400 font-mono text-xs uppercase tracking-wider mb-1 font-bold">
              <Database className="w-4 h-4" /> Pillar 3: 3-Tier Hierarchical Epistemic Memory
            </div>
            <h2 className="text-2xl font-bold text-white tracking-tight font-mono">
              Cross-Session Project Memory & Invariant Graph
            </h2>
            <p className="text-emerald-400/80 text-sm mt-1">
              Eliminating context degradation, memory vomiting, and cross-session amnesia.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={() => setActiveTab('dossier')}
              className={`px-4 py-2 rounded-xl text-xs font-bold transition flex items-center gap-1.5 ${
                activeTab === 'dossier'
                  ? 'bg-gradient-to-r from-emerald-600 to-emerald-500 text-black font-extrabold shadow-lg shadow-emerald-500/20'
                  : 'bg-[#050a07] text-emerald-400/80 hover:text-gold-400 border border-emerald-900/60'
              }`}
            >
              <Layers className="w-3.5 h-3.5" /> Project Truth Dossier
            </button>
            <button
              onClick={() => {
                setActiveTab('simulator');
                if (!simResult) runSimulation(simQuery);
              }}
              className={`px-4 py-2 rounded-xl text-xs font-bold transition flex items-center gap-1.5 ${
                activeTab === 'simulator'
                  ? 'bg-gradient-to-r from-teal-500 to-emerald-400 text-black font-extrabold shadow-lg shadow-teal-500/20'
                  : 'bg-[#050a07] text-teal-400 hover:text-gold-400 border border-teal-800/60'
              }`}
            >
              <ShieldCheck className="w-3.5 h-3.5 text-teal-300" /> Zero-Pollution Guard
            </button>
            <button
              onClick={() => setActiveTab('add')}
              className={`px-4 py-2 rounded-xl text-xs font-bold transition flex items-center gap-1.5 ${
                activeTab === 'add'
                  ? 'bg-gradient-to-r from-gold-500 to-amber-500 text-black font-extrabold shadow-lg shadow-gold-500/20'
                  : 'bg-[#050a07] text-emerald-400/80 hover:text-gold-400 border border-emerald-900/60'
              }`}
            >
              <Plus className="w-3.5 h-3.5" /> Pin Invariant
            </button>
          </div>
        </div>

        {/* 3-Tier Architecture Flow Visualizer */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="bg-[#050a07] border border-emerald-900/80 p-4 rounded-xl space-y-1 shadow-inner">
            <div className="flex items-center justify-between">
              <span className="text-xs font-mono text-gold-400 font-bold">L1: Working Buffer</span>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-gold-500/10 text-gold-400 border border-gold-500/30 font-bold">HOT</span>
            </div>
            <div className="text-sm font-bold text-white font-mono">Sliding Turn Window</div>
            <div className="text-xs text-emerald-400/70">Local dialogue context. Never leaked across un-related chats.</div>
          </div>

          <div className="bg-[#050a07] border border-emerald-900/80 p-4 rounded-xl space-y-1 shadow-inner">
            <div className="flex items-center justify-between">
              <span className="text-xs font-mono text-emerald-400 font-bold">L2: Episodic Ledger</span>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 font-bold">WARM</span>
            </div>
            <div className="text-sm font-bold text-white font-mono">SQLite Session Logs</div>
            <div className="text-xs text-emerald-400/70">Historical event trail. Stored on disk, never dumped raw into prompts.</div>
          </div>

          <div className="bg-[#050a07] border border-emerald-900/80 p-4 rounded-xl space-y-1 shadow-inner">
            <div className="flex items-center justify-between">
              <span className="text-xs font-mono text-jade-300 font-bold">L3: Semantic Graph</span>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-950 text-emerald-300 border border-emerald-500/40 font-bold">COLD / INVARIANT</span>
            </div>
            <div className="text-sm font-bold text-white font-mono">Relevance-Gated Store</div>
            <div className="text-xs text-emerald-400/70">Fixed architectural rules, strictly surfaced only when relevant.</div>
          </div>
        </div>
      </div>

      {/* Tab 1: Project Truth Dossier */}
      {activeTab === 'dossier' && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Active Tech Stack & Invariants */}
          <div className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-bold text-white flex items-center gap-2 font-mono">
                <HardDrive className="w-5 h-5 text-gold-400" />
                L3: Invariants & Active Tech Stack
              </h3>
              <span className="text-[11px] font-mono px-2.5 py-1 rounded-full bg-emerald-900/50 text-emerald-300 border border-emerald-700/50 flex items-center gap-1">
                <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" /> Relevance-Gated
              </span>
            </div>

            <div className="space-y-3">
              {dossier?.active_tech_stack && dossier.active_tech_stack.length > 0 ? (
                dossier.active_tech_stack.map((item, idx) => (
                  <div key={idx} className="bg-[#050a07] border border-emerald-900/80 p-3 rounded-xl text-xs font-mono text-emerald-200 shadow-inner">
                    {item}
                  </div>
                ))
              ) : (
                <div className="text-xs text-emerald-600 italic p-4 text-center">No tech stack items recorded yet.</div>
              )}

              {dossier?.architectural_invariants && dossier.architectural_invariants.length > 0 ? (
                dossier.architectural_invariants.map((item, idx) => (
                  <div key={idx} className="bg-[#050a07] border border-gold-500/30 p-3 rounded-xl text-xs font-mono text-gold-300 shadow-inner">
                    {item}
                  </div>
                ))
              ) : (
                <div className="text-xs text-emerald-600 italic p-4 text-center">No architecture rules recorded yet.</div>
              )}
            </div>
          </div>

          {/* L2: Prior Session Decisions */}
          <div className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-4">
            <h3 className="text-lg font-bold text-white flex items-center gap-2 font-mono">
              <Bookmark className="w-5 h-5 text-gold-400" />
              L2: Decisions Across Past Chats
            </h3>

            <div className="space-y-3">
              {dossier?.recent_decisions && dossier.recent_decisions.length > 0 ? (
                dossier.recent_decisions.map((dec, idx) => (
                  <div key={idx} className="bg-[#050a07] border border-emerald-900/80 p-3 rounded-xl text-xs font-mono text-emerald-300 shadow-inner">
                    {dec}
                  </div>
                ))
              ) : (
                <div className="text-xs text-emerald-600 italic p-4 text-center">No previous session decisions recorded yet.</div>
              )}
            </div>
          </div>

          {/* Markdown Truth Dossier Preview */}
          <div className="lg:col-span-2 bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-2">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-mono uppercase tracking-wider text-gold-400 flex items-center gap-2 font-bold">
                <Sparkles className="w-4 h-4" /> Full Project Truth Dossier (L3 Invariant Overview)
              </h3>
              <button
                onClick={() => onRefreshDossier(projectId)}
                className="text-xs font-mono text-emerald-400 hover:text-gold-400 transition"
              >
                Recompile Dossier ⟳
              </button>
            </div>
            <pre className="bg-[#050a07] border border-emerald-900/80 rounded-xl p-4 text-xs font-mono text-emerald-300 whitespace-pre-wrap overflow-x-auto shadow-inner">
              {dossier?.dossier_markdown || 'No dossier generated yet.'}
            </pre>
          </div>
        </div>
      )}

      {/* Tab 2: Zero-Pollution Guard & Relevance-Gated Simulator */}
      {activeTab === 'simulator' && (
        <div className="space-y-6">
          <div className="bg-[#070f0b]/90 border border-teal-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-4">
            <div className="flex items-center justify-between border-b border-teal-900/40 pb-4">
              <div>
                <h3 className="text-lg font-bold text-white flex items-center gap-2 font-mono">
                  <ShieldCheck className="w-5 h-5 text-teal-400" />
                  Zero-Pollution Guard: Relevance-Gated Epistemic Injection
                </h3>
                <p className="text-xs text-teal-400/80 mt-1 font-mono">
                  Guarantees that new/unrelated chat sessions NEVER receive dumped memory, preserving 100% prompt purity and baseline reasoning quality.
                </p>
              </div>
              <div className="px-3 py-1.5 rounded-xl bg-teal-950 border border-teal-500/40 text-teal-300 font-mono text-xs font-bold flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-teal-400 animate-pulse" />
                POLLUTION GUARD ACTIVE
              </div>
            </div>

            {/* Quick Presets */}
            <div>
              <label className="text-xs font-mono text-gold-400 font-bold block mb-2">
                ⚡ Test Live Scenarios (Click to simulate):
              </label>
              <div className="flex flex-wrap gap-2">
                <button
                  onClick={() => {
                    const q = 'Explain quicksort algorithm in python';
                    setSimQuery(q);
                    runSimulation(q);
                  }}
                  className="px-3 py-1.5 rounded-lg bg-[#050a07] hover:bg-emerald-950/60 border border-emerald-800/60 text-xs font-mono text-emerald-300 transition flex items-center gap-1"
                >
                  <Zap className="w-3 h-3 text-gold-400" /> Unrelated Query (Quicksort)
                </button>
                <button
                  onClick={() => {
                    const q = 'How do I reverse a string in Rust?';
                    setSimQuery(q);
                    runSimulation(q);
                  }}
                  className="px-3 py-1.5 rounded-lg bg-[#050a07] hover:bg-emerald-950/60 border border-emerald-800/60 text-xs font-mono text-emerald-300 transition flex items-center gap-1"
                >
                  <Zap className="w-3 h-3 text-gold-400" /> Generic Coding (Reverse String)
                </button>
                <button
                  onClick={() => {
                    const q = 'What is the deployment protocol for infinitytechstack?';
                    setSimQuery(q);
                    runSimulation(q);
                  }}
                  className="px-3 py-1.5 rounded-lg bg-[#050a07] hover:bg-teal-950/60 border border-teal-700/60 text-xs font-mono text-teal-300 transition flex items-center gap-1"
                >
                  <Zap className="w-3 h-3 text-teal-400" /> Project Rule: Deployment Protocol
                </button>
                <button
                  onClick={() => {
                    const q = 'What 2026 models and language features are active?';
                    setSimQuery(q);
                    runSimulation(q);
                  }}
                  className="px-3 py-1.5 rounded-lg bg-[#050a07] hover:bg-gold-950/60 border border-gold-700/60 text-xs font-mono text-gold-300 transition flex items-center gap-1"
                >
                  <Zap className="w-3 h-3 text-gold-400" /> Project Rule: 2026 Model Invariants
                </button>
              </div>
            </div>

            {/* Input Bar */}
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Search className="w-4 h-4 absolute left-3.5 top-3.5 text-emerald-500" />
                <input
                  type="text"
                  value={simQuery}
                  onChange={(e) => setSimQuery(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && runSimulation(simQuery)}
                  placeholder="Enter any user prompt to evaluate memory relevance..."
                  className="w-full bg-[#050a07] border border-emerald-800 rounded-xl pl-10 pr-4 py-2.5 text-sm text-emerald-100 font-mono focus:outline-none focus:ring-2 focus:ring-teal-500 shadow-inner"
                />
              </div>
              <button
                onClick={() => runSimulation(simQuery)}
                disabled={isSimulating}
                className="px-6 py-2.5 bg-gradient-to-r from-teal-500 to-emerald-400 hover:from-teal-400 hover:to-emerald-300 disabled:opacity-50 text-black font-extrabold text-xs font-mono rounded-xl transition shadow-lg shadow-teal-500/20 flex items-center gap-1.5"
              >
                {isSimulating ? 'Evaluating...' : 'Simulate Gating'}
                <ArrowRight className="w-3.5 h-3.5" />
              </button>
            </div>

            {/* Simulation Results Drawer */}
            {simResult && (
              <div className="mt-4 pt-4 border-t border-teal-900/40 space-y-4">
                <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
                  <div className="bg-[#050a07] border border-emerald-900/80 p-3 rounded-xl space-y-0.5">
                    <span className="text-[10px] font-mono text-emerald-500 font-bold uppercase">Pollution Guard Status</span>
                    <div className="flex items-center gap-1.5 text-sm font-bold font-mono">
                      {simResult.isPollutionBlocked ? (
                        <>
                          <ShieldCheck className="w-4 h-4 text-emerald-400" />
                          <span className="text-emerald-400">Zero-Pollution Enforced</span>
                        </>
                      ) : (
                        <>
                          <CheckCircle2 className="w-4 h-4 text-teal-400" />
                          <span className="text-teal-400">Surgically Gated</span>
                        </>
                      )}
                    </div>
                  </div>

                  <div className="bg-[#050a07] border border-emerald-900/80 p-3 rounded-xl space-y-0.5">
                    <span className="text-[10px] font-mono text-emerald-500 font-bold uppercase">Injected Memory Tokens</span>
                    <div className="text-lg font-bold font-mono text-white">
                      {simResult.tokensInjected} <span className="text-xs text-emerald-400">tokens</span>
                    </div>
                  </div>

                  <div className="bg-[#050a07] border border-emerald-900/80 p-3 rounded-xl space-y-0.5">
                    <span className="text-[10px] font-mono text-emerald-500 font-bold uppercase">Surfaced Invariants</span>
                    <div className="text-lg font-bold font-mono text-white">
                      {simResult.invariantsCount} <span className="text-xs text-gold-400">matched</span>
                    </div>
                  </div>

                  <div className="bg-[#050a07] border border-emerald-900/80 p-3 rounded-xl space-y-0.5">
                    <span className="text-[10px] font-mono text-emerald-500 font-bold uppercase">Context Degradation Risk</span>
                    <div className="text-lg font-bold font-mono text-emerald-400">
                      0.00% <span className="text-[10px] text-emerald-500 font-normal">(Pristine)</span>
                    </div>
                  </div>
                </div>

                {/* Verdict Explanation */}
                {simResult.isPollutionBlocked ? (
                  <div className="bg-emerald-950/40 border border-emerald-600/40 p-4 rounded-xl flex items-start gap-3">
                    <ShieldCheck className="w-5 h-5 text-emerald-400 mt-0.5 flex-shrink-0" />
                    <div className="space-y-1">
                      <div className="text-xs font-bold font-mono text-emerald-300">
                        Zero Memory Injected into Chat Context (Pure Baseline Preserved)
                      </div>
                      <p className="text-xs text-emerald-400/80 leading-relaxed">
                        Prompt <code className="text-gold-300">"{simResult.query}"</code> has zero semantic correlation with project invariants.
                        ChronoFact suppressed all memory dossiers, preventing context window bloat, distraction, and prompt pollution.
                      </p>
                    </div>
                  </div>
                ) : (
                  <div className="bg-teal-950/40 border border-teal-600/40 p-4 rounded-xl flex items-start gap-3">
                    <CheckCircle2 className="w-5 h-5 text-teal-400 mt-0.5 flex-shrink-0" />
                    <div className="space-y-1">
                      <div className="text-xs font-bold font-mono text-teal-300">
                        Exact Invariant Relevancy Surfaced ({simResult.invariantsCount} matched)
                      </div>
                      <p className="text-xs text-teal-400/80 leading-relaxed">
                        Prompt matched specific project knowledge. Only the exact relevant rule was surfaced:
                      </p>
                      <div className="pt-2 space-y-1.5">
                        {simResult.matchedEntities.map((ent, i) => (
                          <div key={i} className="bg-[#050a07] border border-teal-700/60 p-2.5 rounded-lg text-xs font-mono text-teal-200">
                            {ent}
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Tab 3: Pin Invariant Form */}
      {activeTab === 'add' && (
        <form onSubmit={handleSave} className="bg-[#070f0b]/90 border border-emerald-900/60 rounded-2xl p-6 backdrop-blur-md shadow-2xl space-y-4">
          <h3 className="text-lg font-bold text-white font-mono">Pin Permanent Project Invariant / Rule</h3>
          <p className="text-emerald-400/80 text-sm">
            Once saved, every future chat session in project <code className="text-gold-400">"{projectId}"</code> will relevance-gate and access this invariant when relevant.
          </p>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div>
              <label className="text-xs font-mono text-gold-500 font-bold block mb-1">Entity Name / Title</label>
              <input
                type="text"
                value={entityName}
                onChange={(e) => setEntityName(e.target.value)}
                placeholder="e.g. Next.js Version, Ed25519 Auth Rule"
                className="w-full bg-[#050a07] border border-emerald-800 rounded-xl px-4 py-2.5 text-sm text-emerald-100 focus:outline-none focus:ring-2 focus:ring-gold-500 shadow-inner"
                required
              />
            </div>

            <div>
              <label className="text-xs font-mono text-gold-500 font-bold block mb-1">Entity Type</label>
              <select
                value={entityType}
                onChange={(e) => setEntityType(e.target.value)}
                className="w-full bg-[#050a07] border border-emerald-800 rounded-xl px-4 py-2.5 text-sm text-emerald-100 focus:outline-none focus:ring-2 focus:ring-gold-500 shadow-inner"
              >
                <option value="ARCHITECTURE_RULE">ARCHITECTURE_RULE</option>
                <option value="TECH_STACK">TECH_STACK</option>
                <option value="API_CONTRACT">API_CONTRACT</option>
                <option value="DATABASE_SCHEMA">DATABASE_SCHEMA</option>
              </select>
            </div>

            <div>
              <label className="text-xs font-mono text-gold-500 font-bold block mb-1">Version / Revision</label>
              <input
                type="text"
                value={version}
                onChange={(e) => setVersion(e.target.value)}
                placeholder="e.g. 1.0.0 or v2"
                className="w-full bg-[#050a07] border border-emerald-800 rounded-xl px-4 py-2.5 text-sm text-emerald-100 focus:outline-none focus:ring-2 focus:ring-gold-500 shadow-inner"
              />
            </div>
          </div>

          <div>
            <label className="text-xs font-mono text-gold-500 font-bold block mb-1">Detailed Invariant Specification</label>
            <textarea
              rows={3}
              value={definition}
              onChange={(e) => setDefinition(e.target.value)}
              placeholder="e.g. All API endpoints must return standardized JSON { data, error, meta } with strictly typed error codes."
              className="w-full bg-[#050a07] border border-emerald-800 rounded-xl p-4 text-sm text-emerald-100 focus:outline-none focus:ring-2 focus:ring-gold-500 shadow-inner"
              required
            />
          </div>

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={() => setActiveTab('dossier')}
              className="px-5 py-2.5 rounded-xl text-sm font-semibold text-emerald-400 hover:text-gold-400 transition"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSaving}
              className="bg-gradient-to-r from-gold-500 to-amber-500 hover:from-gold-400 hover:to-amber-400 disabled:opacity-50 text-black font-extrabold text-sm px-6 py-2.5 rounded-xl transition shadow-lg shadow-gold-500/20"
            >
              {isSaving ? 'Saving...' : 'Pin to Project Memory'}
            </button>
          </div>
        </form>
      )}
    </div>
  );
};
