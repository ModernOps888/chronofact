import React, { useState } from 'react';
import { ModelHorizon, HorizonAnalysis } from '../types';
import { Clock, ShieldAlert, Sparkles, CheckCircle2, ArrowRight } from 'lucide-react';

interface Props {
  models: ModelHorizon[];
  selectedModel: string;
  onSelectModel: (id: string) => void;
  onRunTest: (query: string) => Promise<HorizonAnalysis | null>;
}

export const TemporalGauge: React.FC<Props> = ({
  models,
  selectedModel,
  onSelectModel,
  onRunTest,
}) => {
  const [testQuery, setTestQuery] = useState('What are the latest updates to GPT-6 Astra, Sol 6.1, and Claude Opus 5.5 in 2026?');
  const [analysis, setAnalysis] = useState<HorizonAnalysis | null>(null);
  const [loading, setLoading] = useState(false);

  const currentModel = models.find((m) => m.model_id === selectedModel) || models[0];

  const handleTest = async () => {
    setLoading(true);
    const res = await onRunTest(testQuery);
    setAnalysis(res);
    setLoading(false);
  };

  return (
    <div className="space-y-6">
      {/* Model Selector Card in Obsidian & Imperial Gold */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/30 rounded-2xl p-6 backdrop-blur-xl shadow-2xl gold-border-glow">
        <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 border-b border-slate-800 pb-6 mb-6">
          <div>
            <div className="flex items-center gap-2 text-amber-400 font-mono text-xs uppercase tracking-wider mb-1 font-bold">
              <Clock className="w-4 h-4 text-amber-400" /> Pillar 1: Temporal Horizon Verification
            </div>
            <h2 className="text-2xl font-black text-white tracking-tight font-mono">
              Knowledge Cutoff & Weight Freeze Matrix
            </h2>
            <p className="text-slate-400 text-sm mt-1">
              Tracking OpenAI (GPT-6 Astra, Sol 6.1), Anthropic (Opus 5.5, Sonnet 5.5), and xAI (Grok 4.7) training freeze windows and dynamic drift.
            </p>
          </div>

          <div className="flex items-center gap-3">
            <label className="text-xs font-mono text-amber-400 font-bold uppercase">Target Model:</label>
            <select
              value={selectedModel}
              onChange={(e) => onSelectModel(e.target.value)}
              className="bg-[#121827] border border-amber-400/50 rounded-xl px-4 py-2.5 text-sm font-bold text-amber-200 focus:outline-none focus:ring-2 focus:ring-amber-400 shadow-lg cursor-pointer"
            >
              {models.map((m) => (
                <option key={m.model_id} value={m.model_id} className="bg-[#0b0f19] text-white">
                  {m.display_name} • {m.vendor} {m.status.includes('Retired') ? '⚠️ RETIRED' : ''}
                </option>
              ))}
            </select>
          </div>
        </div>

        {/* Selected Model Detail Cards */}
        {currentModel && (
          <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
            <div className="bg-[#0f1422] border border-slate-800 p-4 rounded-xl shadow-inner">
              <div className="text-xs font-mono text-slate-400 uppercase font-bold">Public Release</div>
              <div className="text-xl font-black text-white mt-1 font-mono">
                {currentModel.public_release_date}
              </div>
              <div className="text-xs text-amber-400/90 mt-1 font-mono">Vendor: {currentModel.vendor}</div>
            </div>

            <div className="bg-[#0f1422] border border-slate-800 p-4 rounded-xl shadow-inner">
              <div className="text-xs font-mono text-slate-400 uppercase font-bold">Training Freeze</div>
              <div className="text-xl font-black text-amber-400 mt-1 font-mono">
                {currentModel.estimated_training_freeze}
              </div>
              <div className="text-xs text-slate-400 mt-1 font-mono">Static weights freeze date</div>
            </div>

            <div className="bg-[#0f1422] border border-slate-800 p-4 rounded-xl shadow-inner">
              <div className="text-xs font-mono text-slate-400 uppercase font-bold">Official Cutoff</div>
              <div className="text-xl font-black text-amber-300 mt-1 font-mono">
                {currentModel.official_knowledge_cutoff}
              </div>
              <div className="text-xs text-slate-400 mt-1 font-mono">Vendor stated cutoff</div>
            </div>

            <div className="bg-[#0f1422] border border-slate-800 p-4 rounded-xl shadow-inner">
              <div className="text-xs font-mono text-slate-400 uppercase font-bold">Status & Tier</div>
              <div className="text-lg font-black text-white mt-1 font-mono flex items-center gap-1.5">
                <span className={`w-2 h-2 rounded-full ${currentModel.status.includes('Active') ? 'bg-emerald-400' : 'bg-rose-400'}`} />
                {currentModel.status}
              </div>
              <div className="text-xs text-slate-400 mt-1 font-mono truncate">{currentModel.notes}</div>
            </div>
          </div>
        )}
      </div>

      {/* Query Tester Card in Obsidian & Imperial Gold */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/30 rounded-2xl p-6 backdrop-blur-xl shadow-2xl">
        <h3 className="text-lg font-black text-white font-mono flex items-center gap-2 mb-2">
          <Sparkles className="w-5 h-5 text-amber-400" /> Live Query Temporal Drift Simulator
        </h3>
        <p className="text-slate-400 text-sm mb-4">
          Test any user prompt or API specification against {currentModel?.display_name}'s training freeze date to calculate delta days and calibrate prompts.
        </p>

        <div className="flex flex-col sm:flex-row gap-3 mb-6">
          <input
            type="text"
            value={testQuery}
            onChange={(e) => setTestQuery(e.target.value)}
            className="flex-1 bg-[#101626] border border-slate-700 rounded-xl px-4 py-3 text-sm text-white placeholder-slate-500 focus:outline-none focus:border-amber-400"
            placeholder="Enter query to test for temporal sensitivity..."
          />
          <button
            onClick={handleTest}
            disabled={loading}
            className="bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 hover:from-amber-300 hover:to-amber-400 text-black px-6 py-3 rounded-xl font-black text-sm flex items-center justify-center gap-2 shadow-gold-glow cursor-pointer"
          >
            <span>Scan Drift</span>
            <ArrowRight className="w-4 h-4" />
          </button>
        </div>

        {/* Scan Results */}
        {analysis && (
          <div className="space-y-4 border-t border-slate-800 pt-6">
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <div className="bg-[#080b12] border border-slate-800 p-4 rounded-xl">
                <div className="text-xs font-mono text-slate-400">Temporal Risk Score</div>
                <div className="text-3xl font-black text-amber-400 mt-1 font-mono">
                  {(analysis.temporal_risk_score * 100).toFixed(0)}%
                </div>
                <div className="text-xs text-slate-400 mt-1 font-mono">
                  {analysis.temporal_risk_score >= 0.5 ? '⚠️ High Sensitivity' : 'Safe Baseline'}
                </div>
              </div>

              <div className="bg-[#080b12] border border-slate-800 p-4 rounded-xl">
                <div className="text-xs font-mono text-slate-400">Days Past Training Freeze</div>
                <div className="text-3xl font-black text-amber-300 mt-1 font-mono">
                  +{analysis.days_post_freeze} d
                </div>
                <div className="text-xs text-slate-400 mt-1 font-mono">Knowledge gap horizon</div>
              </div>

              <div className="bg-[#080b12] border border-slate-800 p-4 rounded-xl">
                <div className="text-xs font-mono text-slate-400">Grounding Policy</div>
                <div className="text-2xl font-black mt-1 font-mono flex items-center gap-2">
                  {analysis.requires_grounding ? (
                    <span className="text-rose-400 flex items-center gap-1.5">
                      <ShieldAlert className="w-5 h-5 text-rose-400" /> MANDATORY
                    </span>
                  ) : (
                    <span className="text-emerald-400 flex items-center gap-1.5">
                      <CheckCircle2 className="w-5 h-5 text-emerald-400" /> PASS-THROUGH
                    </span>
                  )}
                </div>
                <div className="text-xs text-slate-400 mt-1 font-mono">Search enforcement trigger</div>
              </div>
            </div>

            {/* Generated System Calibration Anchor */}
            <div className="bg-[#080b12] border border-amber-500/30 rounded-xl p-4 font-mono text-xs shadow-inner">
              <div className="text-amber-400 font-bold mb-2 flex items-center gap-1.5">
                <Clock className="w-4 h-4 text-amber-400" /> Injected Epistemic System Calibration Anchor:
              </div>
              <pre className="text-slate-300 whitespace-pre-wrap leading-relaxed overflow-x-auto bg-[#0d121c] p-3 rounded border border-slate-800">
                {analysis.calibration_block}
              </pre>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
