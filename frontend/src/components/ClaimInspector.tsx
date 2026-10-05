import React, { useState } from 'react';
import { VerificationReport, SourceChunk } from '../types';
import { ShieldCheck, FileText, CheckCircle, XCircle, HelpCircle, ExternalLink } from 'lucide-react';

interface Props {
  report: VerificationReport | null;
  sources: SourceChunk[];
  onVerifyCustomText: (text: string) => Promise<void>;
}

export const ClaimInspector: React.FC<Props> = ({ report, sources, onVerifyCustomText }) => {
  const [customText, setCustomText] = useState(
    'Claude 3.5 Sonnet is currently the active recommended model and Grok 3 is the latest flagship.'
  );
  const [loading, setLoading] = useState(false);

  const handleAudit = async () => {
    setLoading(true);
    await onVerifyCustomText(customText);
    setLoading(false);
  };

  return (
    <div className="space-y-6">
      {/* Header Card */}
      <div className="bg-[#0b0f19]/95 border border-amber-500/30 rounded-2xl p-6 backdrop-blur-xl shadow-2xl gold-border-glow">
        <div className="flex items-center gap-2 text-amber-400 font-mono text-xs uppercase tracking-wider mb-1 font-bold">
          <ShieldCheck className="w-4 h-4 text-amber-400" /> Pillar 2: Lexical & Invariant Claim Deconstruction
        </div>
        <h2 className="text-2xl font-black text-white tracking-tight font-mono">
          Atomic Claim Verifier & Invariant Auditor
        </h2>
        <p className="text-slate-400 text-sm mt-1 max-w-3xl">
          Deconstructs model responses into discrete atomic assertions, executes deterministic lexical and invariant checks against retrieved ground-truth sources, and detects factual contradictions.
        </p>

        {/* Custom Text Auditor Input */}
        <div className="mt-6 space-y-3">
          <label className="text-xs font-mono text-amber-400 font-bold uppercase">
            Test LLM Output or Generated Code Commentary:
          </label>
          <textarea
            value={customText}
            onChange={(e) => setCustomText(e.target.value)}
            rows={3}
            className="w-full bg-[#101626] border border-slate-700 rounded-xl p-4 text-sm text-white focus:outline-none focus:border-amber-400 font-mono"
            placeholder="Paste text or claims to verify..."
          />
          <div className="flex justify-end">
            <button
              onClick={handleAudit}
              disabled={loading}
              className="bg-gradient-to-r from-amber-400 via-yellow-400 to-amber-500 hover:from-amber-300 hover:to-amber-400 text-black px-6 py-2.5 rounded-xl font-black text-xs flex items-center gap-2 shadow-gold-glow cursor-pointer"
            >
              <span>{loading ? 'Auditing Claims...' : 'Verify Claims in Rust'}</span>
            </button>
          </div>
        </div>
      </div>

      {/* Verification Metrics Cards */}
      {report && (
        <div className="grid grid-cols-1 sm:grid-cols-4 gap-4">
          <div className="bg-[#0b0f19] border border-slate-800 p-4 rounded-xl shadow-lg">
            <div className="text-xs font-mono text-slate-400">Total Claims</div>
            <div className="text-3xl font-black text-white mt-1 font-mono">{report.total_claims}</div>
            <div className="text-xs text-slate-400 mt-1 font-mono">Atomic propositions</div>
          </div>

          <div className="bg-[#0b0f19] border border-emerald-500/30 p-4 rounded-xl shadow-lg">
            <div className="text-xs font-mono text-emerald-400 font-bold">Entailed (Supported)</div>
            <div className="text-3xl font-black text-emerald-400 mt-1 font-mono">{report.entailed_count}</div>
            <div className="text-xs text-slate-400 mt-1 font-mono">Verified by ground truth</div>
          </div>

          <div className="bg-[#0b0f19] border border-rose-500/30 p-4 rounded-xl shadow-lg">
            <div className="text-xs font-mono text-rose-400 font-bold">Contradicted (Clashes)</div>
            <div className="text-3xl font-black text-rose-400 mt-1 font-mono">{report.contradicted_count}</div>
            <div className="text-xs text-slate-400 mt-1 font-mono">Hallucinations / clashes</div>
          </div>

          <div className="bg-[#0b0f19] border border-amber-500/30 p-4 rounded-xl shadow-lg">
            <div className="text-xs font-mono text-amber-400 font-bold">Hallucination Risk</div>
            <div className="text-3xl font-black text-amber-400 mt-1 font-mono">
              {(report.hallucination_risk_index * 100).toFixed(0)}%
            </div>
            <div className="text-xs text-slate-400 mt-1 font-mono">Aggregate epistemic risk</div>
          </div>
        </div>
      )}

      {/* Claims Breakdown List */}
      {report && report.claims.length > 0 && (
        <div className="space-y-3">
          <h3 className="text-sm font-bold font-mono text-amber-300 uppercase tracking-wider">
            Atomic Claims Verification Details
          </h3>
          <div className="space-y-3">
            {report.claims.map((vc, i) => (
              <div
                key={i}
                className={`bg-[#0c101c] border p-4.5 rounded-xl shadow-lg ${
                  vc.status === 'Entailed'
                    ? 'border-emerald-500/30'
                    : vc.status === 'Contradicted'
                    ? 'border-rose-500/40 bg-rose-950/10'
                    : 'border-slate-800'
                }`}
              >
                <div className="flex flex-wrap justify-between items-start gap-2 mb-2">
                  <div className="flex items-center gap-2">
                    {vc.status === 'Entailed' && <CheckCircle className="w-4 h-4 text-emerald-400" />}
                    {vc.status === 'Contradicted' && <XCircle className="w-4 h-4 text-rose-400" />}
                    {vc.status === 'Unverified' && <HelpCircle className="w-4 h-4 text-amber-400" />}
                    <span className="font-mono text-xs font-bold text-slate-400">
                      Claim #{i + 1} • <strong className="text-white">[{vc.claim.category}]</strong>
                    </span>
                  </div>
                  <span
                    className={`text-[10px] font-mono px-2 py-0.5 rounded font-bold ${
                      vc.status === 'Entailed'
                        ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40'
                        : vc.status === 'Contradicted'
                        ? 'bg-rose-500/20 text-rose-300 border border-rose-500/40'
                        : 'bg-amber-500/20 text-amber-300 border border-amber-500/40'
                    }`}
                  >
                    {vc.status.toUpperCase()} ({(vc.confidence_score * 100).toFixed(0)}% Conf)
                  </span>
                </div>

                <div className="text-sm text-slate-200 font-mono font-semibold mb-2">
                  "{vc.claim.statement}"
                </div>

                <div className="text-xs text-slate-400 font-mono bg-[#080b12] p-2.5 rounded border border-slate-800/80">
                  <span className="text-amber-400 font-bold">Verification Rationale:</span> {vc.rationale}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Grounding Source Chunks */}
      {sources.length > 0 && (
        <div className="bg-[#0b0f19] border border-slate-800 rounded-2xl p-6 shadow-xl">
          <h3 className="text-sm font-bold font-mono text-emerald-400 uppercase tracking-wider mb-4 flex items-center gap-2">
            <FileText className="w-4 h-4 text-emerald-400" /> Active Grounded Web Evidence ({sources.length})
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {sources.map((s) => (
              <div key={s.id} className="bg-[#080b12] border border-slate-800/80 p-4 rounded-xl">
                <div className="flex items-center justify-between text-xs font-mono text-amber-400 mb-1">
                  <span className="font-bold truncate max-w-xs">{s.title}</span>
                  <a
                    href={s.url}
                    target="_blank"
                    rel="noreferrer"
                    className="text-slate-400 hover:text-amber-300 flex items-center gap-1"
                  >
                    <ExternalLink className="w-3 h-3" />
                  </a>
                </div>
                <div className="text-xs text-slate-300 line-clamp-3 font-mono leading-relaxed mt-1">
                  {s.content}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
