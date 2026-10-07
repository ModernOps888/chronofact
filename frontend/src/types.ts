export interface ModelHorizon {
  model_id: string;
  display_name: string;
  vendor: string;
  public_release_date: string;
  estimated_training_freeze: string;
  official_knowledge_cutoff: string;
  is_frontier: boolean;
  status: string;
  notes: string;
}

export interface HorizonAnalysis {
  current_date: string;
  model_id: string;
  model_name: string;
  official_cutoff: string;
  training_freeze: string;
  days_post_cutoff: number;
  days_post_freeze: number;
  temporal_risk_score: number;
  requires_grounding: boolean;
  calibration_block: string;
}

export interface SourceChunk {
  id: string;
  title: string;
  url: string;
  content: string;
  integrity_hash: string;
  is_sanitized: boolean;
}

export interface AtomicClaim {
  id: string;
  statement: string;
  category: 'TechnicalApi' | 'VersionCompatibility' | 'TemporalEvent' | 'FactualAssertion' | 'General';
}

export interface VerifiedClaim {
  claim: AtomicClaim;
  status: 'Entailed' | 'Contradicted' | 'Unverified';
  confidence_score: number;
  matched_source_ids: string[];
  rationale: string;
}

export interface VerificationReport {
  total_claims: number;
  entailed_count: number;
  contradicted_count: number;
  unverified_count: number;
  hallucination_risk_index: number;
  claims: VerifiedClaim[];
}

export interface ProjectEntity {
  id: string;
  project_id: string;
  entity_name: string;
  entity_type: string;
  definition: string;
  version: string;
  updated_at: string;
}

export interface ProjectDossier {
  project_id: string;
  generated_at: string;
  active_tech_stack: string[];
  architectural_invariants: string[];
  recent_decisions: string[];
  dossier_markdown: string;
}

export interface EpistemicChatResponse {
  project_id: string;
  model_id: string;
  answer: string;
  temporal_analysis: HorizonAnalysis;
  grounding_sources: SourceChunk[];
  verification_report: VerificationReport;
  project_dossier: ProjectDossier;
  security_shields_active: boolean;
}

export interface SecurityAuditReport {
  status: string;
  ssrf_firewall_enabled: boolean;
  prompt_injection_shield_enabled: boolean;
  sql_parameterization_enforced: boolean;
  memory_isolation_active: boolean;
  timestamp: string;
  total_audited_vectors: number;
}

export interface DriftEvent {
  id: number;
  timestamp: string;
  project_id: string;
  model_id: string;
  query: string;
  days_post_freeze: number;
  temporal_risk_score: number;
  outdated_topics_caught: string;
  intervention_type: string;
  ground_truth_retrieved: string;
}

export interface CostMetricsSummary {
  total_queries_optimized: number;
  total_tools_pruned: number;
  total_tokens_saved: number;
  estimated_usd_saved: number;
  cache_hits: number;
  cache_misses: number;
  cache_hit_rate_pct: number;
  average_tokens_saved_per_turn: number;
  active_cache_entries: number;
  prompt_caching_aligned: boolean;
}

export interface ToolCandidate {
  name: string;
  description: string;
  category: string;
  parameters: string[];
}

export interface ToolRoutingResult {
  query: string;
  selected_tools: ToolCandidate[];
  pruned_tools: number;
  tokens_saved: number;
  cost_reduction_pct: number;
  prompt_cache_anchor: string;
}

export interface GatewayServerStatus {
  name: string;
  connected: boolean;
  transport: string;
  tools_count: number;
  resources_count: number;
  prompts_count: number;
}

export interface GatewayTool {
  definition: {
    name: string;
    description?: string;
    inputSchema?: any;
    annotations?: {
      readOnlyHint?: boolean;
      readOnly?: boolean;
    };
  };
  server_name: string;
  fqn: string;
}

