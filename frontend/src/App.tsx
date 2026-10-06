import React, { useState, useEffect } from 'react';
import { ModelHorizon, ProjectDossier, VerificationReport, SourceChunk, HorizonAnalysis, EpistemicChatResponse, SecurityAuditReport, DriftEvent } from './types';
import { EpistemicChat } from './components/EpistemicChat';
import { TemporalGauge } from './components/TemporalGauge';
import { ClaimInspector } from './components/ClaimInspector';
import { MemoryRadar } from './components/MemoryRadar';
import { SecurityAuditView } from './components/SecurityAuditView';
import { LiveDriftFeed } from './components/LiveDriftFeed';
import { CostOptimizerHUD } from './components/CostOptimizerHUD';
import { Sparkles, Clock, ShieldCheck, Database, Lock, MessageSquare, Activity, Coins } from 'lucide-react';

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'drift' | 'chat' | 'temporal' | 'claims' | 'memory' | 'security' | 'cost'>('cost');
  const [models, setModels] = useState<ModelHorizon[]>([]);
  const [selectedModel, setSelectedModel] = useState('gpt-6-astra');
  const [projectId, setProjectId] = useState('antigravity-ide');
  const [dossier, setDossier] = useState<ProjectDossier | null>(null);
  const [auditReport, setAuditReport] = useState<SecurityAuditReport | null>(null);
  const [lastReport, setLastReport] = useState<VerificationReport | null>(null);
  const [lastSources, setLastSources] = useState<SourceChunk[]>([]);
  const [driftEvents, setDriftEvents] = useState<DriftEvent[]>([]);
  const [serverOnline, setServerOnline] = useState(false);

  useEffect(() => {
    fetchInitialData();
  }, [projectId]);

  const fetchInitialData = async () => {
    try {
      const healthRes = await fetch('/api/health');
      if (healthRes.ok) setServerOnline(true);

      const modelsRes = await fetch('/api/models');
      if (modelsRes.ok) {
        const data = await modelsRes.json();
        setModels(data);
      }

      const dossierRes = await fetch(`/api/memory/dossier/${projectId}`);
      if (dossierRes.ok) {
        const data = await dossierRes.json();
        setDossier(data);
      }

      const auditRes = await fetch('/api/security/audit');
      if (auditRes.ok) {
        const data = await auditRes.json();
        setAuditReport(data);
      }

      await fetchDriftEvents();
    } catch (err) {
      console.warn('Backend not responding yet:', err);
      setServerOnline(false);
    }
  };

  const fetchDriftEvents = async () => {
    try {
      const res = await fetch('/api/drift/events');
      if (res.ok) {
        const events = await res.json();
        setDriftEvents(events);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const handleSendMessage = async (query: string, modelId: string, projId: string): Promise<EpistemicChatResponse | null> => {
    try {
      const res = await fetch('/api/chat/epistemic', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          project_id: projId,
          model_id: modelId,
          user_query: query,
        }),
      });

      if (!res.ok) throw new Error('API error');
      const data: EpistemicChatResponse = await res.json();
      setLastReport(data.verification_report);
      setLastSources(data.grounding_sources);
      setDossier(data.project_dossier);
      // Immediately refresh the live drift feed so the caught outdated info shows up instantly!
      await fetchDriftEvents();
      return data;
    } catch (err) {
      console.error(err);
      return null;
    }
  };

  const handleTestTemporal = async (query: string): Promise<HorizonAnalysis | null> => {
    try {
      const res = await fetch('/api/temporal/check', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model_id: selectedModel, query }),
      });
      if (res.ok) return await res.json();
      return null;
    } catch {
      return null;
    }
  };

  const handleVerifyCustomText = async (text: string) => {
    try {
      const res = await fetch('/api/grounding/verify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ response_text: text, sources: lastSources }),
      });
      if (res.ok) {
        const data = await res.json();
        setLastReport(data);
      }
    } catch (err) {
      console.error(err);
    }
  };

  const handleSaveEntity = async (entity: {
    projectId: string;
    entityName: string;
    entityType: string;
    definition: string;
    version?: string;
  }) => {
    try {
      await fetch('/api/memory/entity', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          project_id: entity.projectId,
          entity_name: entity.entityName,
          entity_type: entity.entityType,
          definition: entity.definition,
          version: entity.version,
        }),
      });
    } catch (err) {
      console.error(err);
    }
  };

  const activeModelObj = models.find(m => m.model_id === selectedModel);

  return (
    <div className="min-h-screen bg-[#07090e] text-slate-100 flex flex-col font-sans selection:bg-amber-400 selection:text-black">
      {/* Top Header in Polished Obsidian & Imperial Gold */}
      <header className="border-b border-amber-500/25 bg-[#0b0f19]/90 backdrop-blur-xl sticky top-0 z-50 px-6 py-4 shadow-2xl">
        <div className="max-w-7xl mx-auto flex flex-wrap justify-between items-center gap-4">
          <div className="flex items-center gap-4">
            <div className="w-11 h-11 rounded-2xl bg-gradient-to-tr from-amber-500 via-yellow-400 to-amber-200 flex items-center justify-center shadow-gold-glow border border-amber-300">
              <Sparkles className="w-6 h-6 text-black font-black" />
            </div>
            <div>
              <div className="flex items-center gap-3">
                <h1 className="text-2xl font-black tracking-tight text-transparent bg-clip-text bg-gradient-to-r from-amber-200 via-amber-400 to-yellow-300 font-mono gold-text-glow">
                  ChronoFact
                </h1>
                <span className="text-[11px] font-mono px-2.5 py-0.5 rounded-full bg-amber-500/10 text-amber-300 border border-amber-400/30 font-bold shadow-inner flex items-center gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>
                  IMPERIAL GOLD & JEWEL EMERALD
                </span>
              </div>
              <p className="text-xs text-slate-400 font-mono mt-0.5">
                Epistemic AI Backbone • Training Cutoff Interception • Factual Grounding • Invariant Memory
              </p>
            </div>
          </div>

          {/* Project & Model Switcher with High-Contrast Cards */}
          <div className="flex flex-wrap items-center gap-3">
            <div className="flex items-center bg-[#101626] border border-amber-500/25 rounded-xl px-3.5 py-2 text-xs font-mono shadow-sm">
              <span className="text-amber-400 mr-2.5 font-bold">PROJECT:</span>
              <input
                type="text"
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
                className="bg-transparent text-white focus:outline-none w-32 font-bold tracking-wide"
                placeholder="project-id"
              />
            </div>

            <div className="flex items-center bg-[#101626] border border-amber-500/25 rounded-xl px-3.5 py-2 text-xs font-mono shadow-sm">
              <span className="text-amber-400 mr-2.5 font-bold">MODEL:</span>
              <select
                value={selectedModel}
                onChange={(e) => setSelectedModel(e.target.value)}
                className="bg-transparent text-amber-200 font-bold focus:outline-none cursor-pointer"
              >
                {models.map((m) => (
                  <option key={m.model_id} value={m.model_id} className="bg-[#0b0f19] text-white">
                    {m.display_name} • {m.vendor} {m.status.includes('Retired') ? '⚠️ RETIRED' : ''}
                  </option>
                ))}
              </select>
            </div>

            <div className={`flex items-center gap-2 bg-[#101626] border px-3.5 py-2 rounded-xl shadow-sm ${
              serverOnline ? 'border-emerald-500/30' : 'border-rose-500/40'
            }`}>
              <span
                className={`w-2.5 h-2.5 rounded-full ${
                  serverOnline ? 'bg-emerald-400 shadow-[0_0_10px_#10b981] animate-pulse' : 'bg-rose-500'
                }`}
              />
              <span className={`text-xs font-mono font-semibold ${
                serverOnline ? 'text-emerald-300' : 'text-rose-300'
              }`}>
                {serverOnline ? 'Core 127.0.0.1:3030' : 'Offline'}
              </span>
            </div>
          </div>
        </div>

        {/* Dynamic Model Horizon Bar */}
        {activeModelObj && (
          <div className="max-w-7xl mx-auto mt-3 pt-3 border-t border-slate-800/80 flex flex-wrap items-center justify-between text-xs font-mono text-slate-300">
            <div className="flex items-center gap-4 flex-wrap">
              <span className="text-amber-300 font-bold">{activeModelObj.display_name}</span>
              <span className="text-slate-400">Vendor: <strong className="text-white">{activeModelObj.vendor}</strong></span>
              <span className="text-slate-400">Cutoff: <strong className="text-amber-300">{activeModelObj.official_knowledge_cutoff}</strong></span>
              <span className="text-slate-400">Freeze: <strong className="text-amber-400">{activeModelObj.estimated_training_freeze}</strong></span>
              <span className={`px-2 py-0.5 rounded text-[10px] font-bold ${
                activeModelObj.status.includes('Active')
                  ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40'
                  : activeModelObj.status.includes('Retired')
                  ? 'bg-rose-500/20 text-rose-300 border border-rose-500/40'
                  : 'bg-amber-500/20 text-amber-300 border border-amber-500/40'
              }`}>
                {activeModelObj.status}
              </span>
            </div>
            <div className="text-slate-400 truncate max-w-md hidden md:block">
              {activeModelObj.notes}
            </div>
          </div>
        )}
      </header>

      {/* Radiant Tab Navigation Bar */}
      <div className="border-b border-amber-500/20 bg-[#090d16]/80 px-6 backdrop-blur-md">
        <div className="max-w-7xl mx-auto flex gap-2.5 overflow-x-auto py-3">
          <button
            onClick={() => setActiveTab('drift')}
            className={`px-4 py-2.5 rounded-xl text-xs font-bold transition-all flex items-center gap-2 ${
              activeTab === 'drift'
                ? 'bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 text-black font-black shadow-gold-glow scale-[1.02]'
                : 'text-slate-300 hover:text-amber-300 hover:bg-[#121827] border border-transparent hover:border-amber-500/20'
            }`}
          >
            <Activity className="w-4 h-4" /> Live Outdated Interceptor ({driftEvents.length})
          </button>

          <button
            onClick={() => setActiveTab('chat')}
            className={`px-4 py-2.5 rounded-xl text-xs font-bold transition-all flex items-center gap-2 ${
              activeTab === 'chat'
                ? 'bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 text-black font-black shadow-gold-glow scale-[1.02]'
                : 'text-slate-300 hover:text-amber-300 hover:bg-[#121827] border border-transparent hover:border-amber-500/20'
            }`}
          >
            <MessageSquare className="w-4 h-4" /> Epistemic Chat
          </button>

          <button
            onClick={() => setActiveTab('temporal')}
            className={`px-4 py-2.5 rounded-xl text-xs font-bold transition-all flex items-center gap-2 ${
              activeTab === 'temporal'
                ? 'bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 text-black font-black shadow-gold-glow scale-[1.02]'
                : 'text-slate-300 hover:text-amber-300 hover:bg-[#121827] border border-transparent hover:border-amber-500/20'
            }`}
          >
            <Clock className="w-4 h-4" /> Temporal Horizon Radar
          </button>

          <button
            onClick={() => setActiveTab('claims')}
            className={`px-4 py-2.5 rounded-xl text-xs font-bold transition-all flex items-center gap-2 ${
              activeTab === 'claims'
                ? 'bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 text-black font-black shadow-gold-glow scale-[1.02]'
                : 'text-slate-300 hover:text-amber-300 hover:bg-[#121827] border border-transparent hover:border-amber-500/20'
            }`}
          >
            <ShieldCheck className="w-4 h-4" /> Claim Verifier
          </button>

          <button
            onClick={() => setActiveTab('memory')}
            className={`px-4 py-2.5 rounded-xl text-xs font-bold transition-all flex items-center gap-2 ${
              activeTab === 'memory'
                ? 'bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 text-black font-black shadow-gold-glow scale-[1.02]'
                : 'text-slate-300 hover:text-amber-300 hover:bg-[#121827] border border-transparent hover:border-amber-500/20'
            }`}
          >
            <Database className="w-4 h-4" /> Memory Radar
          </button>

          <button
            onClick={() => setActiveTab('security')}
            className={`px-4 py-2.5 rounded-xl text-xs font-bold transition-all flex items-center gap-2 ${
              activeTab === 'security'
                ? 'bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 text-black font-black shadow-gold-glow scale-[1.02]'
                : 'text-slate-300 hover:text-amber-300 hover:bg-[#121827] border border-transparent hover:border-amber-500/20'
            }`}
          >
            <Lock className="w-4 h-4" /> Security Audit
          </button>

          <button
            onClick={() => setActiveTab('cost')}
            className={`px-4 py-2.5 rounded-xl text-xs font-bold transition-all flex items-center gap-2 ${
              activeTab === 'cost'
                ? 'bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 text-black font-black shadow-gold-glow scale-[1.02]'
                : 'text-slate-300 hover:text-amber-300 hover:bg-[#121827] border border-transparent hover:border-amber-500/20'
            }`}
          >
            <Coins className="w-4 h-4" /> Cost & Token Optimizer
          </button>
        </div>
      </div>

      {/* Main Content Area with Obsidian Depth and Gold Framing */}
      <main className="flex-1 max-w-7xl w-full mx-auto p-6 md:p-8">
        {activeTab === 'drift' && (
          <LiveDriftFeed events={driftEvents} onRefresh={fetchDriftEvents} />
        )}

        {activeTab === 'chat' && (
          <EpistemicChat
            selectedModel={selectedModel}
            projectId={projectId}
            onSendMessage={handleSendMessage}
          />
        )}

        {activeTab === 'temporal' && (
          <TemporalGauge
            models={models}
            selectedModel={selectedModel}
            onSelectModel={setSelectedModel}
            onRunTest={handleTestTemporal}
          />
        )}

        {activeTab === 'claims' && (
          <ClaimInspector
            report={lastReport}
            sources={lastSources}
            onVerifyCustomText={handleVerifyCustomText}
          />
        )}

        {activeTab === 'memory' && (
          <MemoryRadar
            projectId={projectId}
            dossier={dossier}
            onRefreshDossier={async (id) => {
              const res = await fetch(`/api/memory/dossier/${id}`);
              if (res.ok) setDossier(await res.json());
            }}
            onSaveEntity={handleSaveEntity}
          />
        )}

        {activeTab === 'security' && (
          <SecurityAuditView audit={auditReport} />
        )}

        {activeTab === 'cost' && (
          <CostOptimizerHUD />
        )}
      </main>

      {/* Golden Footer Bar */}
      <footer className="border-t border-amber-500/20 bg-[#070a10] py-4 px-6 text-center text-xs font-mono text-slate-400">
        <div className="max-w-7xl mx-auto flex flex-col sm:flex-row justify-between items-center gap-2">
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-amber-400 shadow-[0_0_8px_#f59e0b]"></span>
            <span>ChronoFact Epistemic Architecture • Rust Core + React/TypeScript Cockpit</span>
          </div>
          <div className="text-amber-400/90 font-semibold">
            Grounded Horizon Evaluation: <span className="text-white">2026-10-05</span>
          </div>
        </div>
      </footer>
    </div>
  );
};
