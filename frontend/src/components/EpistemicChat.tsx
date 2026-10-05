import React, { useState } from 'react';
import { EpistemicChatResponse } from '../types';
import { Send, Bot, User, Clock, ShieldCheck, Database, Sparkles, ChevronDown, ChevronUp } from 'lucide-react';

interface Props {
  selectedModel: string;
  projectId: string;
  onSendMessage: (query: string, modelId: string, projectId: string) => Promise<EpistemicChatResponse | null>;
}

interface MessageItem {
  role: 'user' | 'assistant';
  content: string;
  telemetry?: EpistemicChatResponse;
}

export const EpistemicChat: React.FC<Props> = ({
  selectedModel,
  projectId,
  onSendMessage,
}) => {
  const [messages, setMessages] = useState<MessageItem[]>([
    {
      role: 'assistant',
      content:
        '🏛️ Welcome to **Project ChronoFact Epistemic Console**.\n\nI am wired directly to the Rust backend and calibrated for 2026. Ask me about frontier models (**GPT-6 Astra, GPT-6.1 Sol, Claude Opus 5.5, Claude Sonnet 5.5, Grok 4.7**), 2026 library releases, or persistent project invariants.\n\nChronoFact intercepts training freeze boundaries, pulls live web evidence, and deconstructs every claim into atomic propositions to guarantee zero hallucination and zero memory amnesia.',
    },
  ]);
  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [expandedTelemetry, setExpandedTelemetry] = useState<number | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || loading) return;

    const userText = input;
    setInput('');
    setMessages((prev) => [...prev, { role: 'user', content: userText }]);
    setLoading(true);

    const res = await onSendMessage(userText, selectedModel, projectId);

    if (res) {
      setMessages((prev) => [
        ...prev,
        {
          role: 'assistant',
          content: res.answer,
          telemetry: res,
        },
      ]);
      setExpandedTelemetry(messages.length + 1);
    } else {
      setMessages((prev) => [
        ...prev,
        {
          role: 'assistant',
          content: '⚠️ Failed to connect to the ChronoFact Rust Backbone. Ensure `chronofact serve` is running on port 3030.',
        },
      ]);
    }

    setLoading(false);
  };

  return (
    <div className="flex flex-col h-[750px] bg-[#0c101c]/95 border border-amber-500/25 rounded-2xl backdrop-blur-xl shadow-2xl overflow-hidden gold-border-glow">
      {/* Chat Header in Obsidian & Imperial Gold */}
      <div className="bg-[#090d16] border-b border-amber-500/20 px-6 py-4 flex flex-wrap justify-between items-center gap-4">
        <div className="flex items-center gap-3.5">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-tr from-amber-500 via-yellow-400 to-amber-200 flex items-center justify-center shadow-gold-glow border border-amber-300">
            <Sparkles className="w-5 h-5 text-black font-black" />
          </div>
          <div>
            <h2 className="text-sm font-black text-amber-400 tracking-wider font-mono uppercase gold-text-glow">
              Epistemic Interactive Console
            </h2>
            <div className="text-[11px] font-mono text-slate-400 flex items-center gap-2 mt-0.5">
              <span>Active Model: <strong className="text-amber-300 font-bold">{selectedModel}</strong></span>
              <span>•</span>
              <span>Project ID: <strong className="text-emerald-400 font-bold">{projectId}</strong></span>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <span className="inline-flex items-center gap-1.5 text-xs font-mono px-3.5 py-1.5 rounded-full bg-emerald-500/15 text-emerald-300 border border-emerald-500/40 shadow-inner font-bold">
            <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
            3 Epistemic Pillars Armed
          </span>
        </div>
      </div>

      {/* Messages Scroll Area */}
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {messages.map((m, idx) => (
          <div key={idx} className={`flex gap-3.5 ${m.role === 'user' ? 'justify-end' : 'justify-start'}`}>
            {m.role === 'assistant' && (
              <div className="w-9 h-9 rounded-xl bg-[#141b2c] border border-amber-500/40 flex items-center justify-center shrink-0 text-amber-400 shadow-md">
                <Bot className="w-5 h-5" />
              </div>
            )}

            <div className={`max-w-2xl space-y-2 ${m.role === 'user' ? 'items-end' : 'items-start'}`}>
              <div
                className={`p-4.5 rounded-2xl text-sm leading-relaxed shadow-lg ${
                  m.role === 'user'
                    ? 'bg-gradient-to-r from-amber-500 via-amber-400 to-yellow-400 text-black font-bold rounded-tr-sm border border-amber-300 shadow-gold-glow'
                    : 'bg-[#101626] border border-slate-700/80 text-slate-100 rounded-tl-sm'
                }`}
              >
                <div className="whitespace-pre-wrap">{m.content}</div>
              </div>

              {/* Epistemic Telemetry Drawer */}
              {m.telemetry && (
                <div className="w-full bg-[#080b12] border border-amber-500/30 rounded-xl p-3.5 text-xs space-y-2.5 shadow-inner">
                  <button
                    onClick={() => setExpandedTelemetry(expandedTelemetry === idx ? null : idx)}
                    className="w-full flex items-center justify-between text-amber-400 hover:text-amber-300 font-mono font-bold cursor-pointer"
                  >
                    <span className="flex items-center gap-2">
                      <ShieldCheck className="w-4 h-4 text-emerald-400" />
                      Verification Telemetry: {m.telemetry.verification_report.entailed_count} Verified Claims • Hallucination Risk: {(m.telemetry.verification_report.hallucination_risk_index * 100).toFixed(0)}%
                    </span>
                    {expandedTelemetry === idx ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                  </button>

                  {expandedTelemetry === idx && (
                    <div className="mt-2.5 pt-2.5 border-t border-slate-800 space-y-3 font-mono text-[11px]">
                      {/* Pillar 1 */}
                      <div className="space-y-1">
                        <div className="text-amber-400 font-bold flex items-center gap-1.5">
                          <Clock className="w-3.5 h-3.5 text-amber-400" /> Pillar 1: Temporal Horizon Delta
                        </div>
                        <div className="text-slate-300 pl-5">
                          Cutoff: <strong className="text-amber-300">{m.telemetry.temporal_analysis.official_cutoff}</strong> (Freeze: <strong className="text-amber-400">{m.telemetry.temporal_analysis.training_freeze}</strong>) • Delta: <span className="text-amber-400 font-bold">+{m.telemetry.temporal_analysis.days_post_freeze} Days Beyond Freeze</span>
                        </div>
                      </div>

                      {/* Pillar 2 */}
                      <div className="space-y-1">
                        <div className="text-emerald-400 font-bold flex items-center gap-1.5">
                          <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" /> Pillar 2: Active Grounding & Truth Verification
                        </div>
                        <div className="text-slate-300 pl-5">
                          Sources Fetched: <strong className="text-white">{m.telemetry.grounding_sources.length}</strong> • Contradictions: <strong className={m.telemetry.verification_report.contradicted_count > 0 ? 'text-rose-400' : 'text-emerald-400'}>{m.telemetry.verification_report.contradicted_count}</strong>
                        </div>
                      </div>

                      {/* Pillar 3 */}
                      <div className="space-y-1">
                        <div className="text-amber-400 font-bold flex items-center gap-1.5">
                          <Database className="w-3.5 h-3.5 text-amber-400" /> Pillar 3: Project Truth Dossier Injected
                        </div>
                        <div className="text-slate-300 pl-5">
                          Architectural Invariants: <strong className="text-white">{m.telemetry.project_dossier.architectural_invariants.length}</strong> • Prior Decisions: <strong className="text-white">{m.telemetry.project_dossier.recent_decisions.length}</strong>
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>

            {m.role === 'user' && (
              <div className="w-9 h-9 rounded-xl bg-amber-500/20 border border-amber-400/40 flex items-center justify-center shrink-0 text-amber-300 shadow-md">
                <User className="w-5 h-5" />
              </div>
            )}
          </div>
        ))}

        {loading && (
          <div className="flex gap-3 items-center text-amber-400 text-xs font-mono">
            <div className="w-9 h-9 rounded-xl bg-[#141b2c] border border-amber-500/40 flex items-center justify-center animate-pulse">
              <Bot className="w-5 h-5 text-amber-400" />
            </div>
            <span>Evaluating temporal drift, retrieving live sources, and verifying claims in Rust...</span>
          </div>
        )}
      </div>

      {/* Input Area in Obsidian & Imperial Gold */}
      <form onSubmit={handleSubmit} className="p-4 bg-[#090d16] border-t border-amber-500/20 flex gap-3">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder={`Ask about GPT-6 Astra, Sol 6.1, Claude Opus 5.5, or project ${projectId}...`}
          className="flex-1 bg-[#101626] border border-amber-500/30 rounded-xl px-4.5 py-3 text-sm text-white placeholder-slate-500 focus:outline-none focus:border-amber-400 focus:ring-1 focus:ring-amber-400"
        />
        <button
          type="submit"
          disabled={loading || !input.trim()}
          className="bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 hover:from-amber-300 hover:to-amber-400 disabled:opacity-50 text-black px-6 py-3 rounded-xl transition font-black text-sm flex items-center gap-2 shadow-gold-glow cursor-pointer"
        >
          <Send className="w-4 h-4" />
          <span>Send</span>
        </button>
      </form>
    </div>
  );
};
