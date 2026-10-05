use super::registry::ModelHorizon;
use super::scanner::TemporalScanResult;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorizonAnalysis {
    pub current_date: NaiveDate,
    pub model_id: String,
    pub model_name: String,
    pub model_status: String,
    pub official_cutoff: NaiveDate,
    pub training_freeze: NaiveDate,
    pub days_post_cutoff: i64,
    pub days_post_freeze: i64,
    pub temporal_risk_score: f32,
    pub is_model_outdated_or_retired: bool,
    pub recommended_replacement: Option<String>,
    pub outdated_warnings: Vec<String>,
    pub requires_grounding: bool,
    pub calibration_block: String,
}

pub struct HorizonCalculator;

impl HorizonCalculator {
    pub fn evaluate(
        model: &ModelHorizon,
        scan: &TemporalScanResult,
        current_date: NaiveDate,
    ) -> HorizonAnalysis {
        let days_post_cutoff = (current_date - model.official_knowledge_cutoff).num_days();
        let days_post_freeze = (current_date - model.estimated_training_freeze).num_days();

        let is_retired = model.status.contains("Retired")
            || model.status.contains("Deprecated")
            || model.status.contains("Superseded");

        let recommended_replacement = if model.model_id.contains("3-5") || model.model_id.contains("3-7") {
            Some("Claude Sonnet 5.5 / Claude Opus 5.5".to_string())
        } else if model.model_id.contains("grok-3") || model.model_id.contains("grok-2") {
            Some("Grok 4.7 (xAI September 2026)".to_string())
        } else if model.model_id.contains("gpt-4") {
            Some("GPT-5 / OpenAI o3".to_string())
        } else {
            None
        };

        let mut outdated_warnings = scan.outdated_models_flagged.clone();
        if is_retired {
            outdated_warnings.push(format!(
                "Selected model '{}' is {}! {}",
                model.display_name, model.status, model.notes
            ));
        }

        // If the query touches recent years or entities, and the model is post-freeze:
        let is_post_freeze_query = scan.mentioned_years.iter().any(|&yr| {
            let model_freeze_year = model.estimated_training_freeze.format("%Y").to_string().parse::<i32>().unwrap_or(2024);
            yr >= model_freeze_year
        });

        let requires_grounding = scan.requires_search
            || (scan.is_temporally_sensitive && days_post_freeze > 60)
            || is_post_freeze_query
            || is_retired;

        let warning_section = if !outdated_warnings.is_empty() {
            format!(
                "\n<CRITICAL_OUTDATED_MODEL_ALERT>\n\
                WARNING: OUTDATED/RETIRED MODEL BELIEFS DETECTED!\n\
                {}\n\
                Active Recommended Successors: {}\n\
                DO NOT generate advice relying on retired models or frozen knowledge boundaries.\n\
                </CRITICAL_OUTDATED_MODEL_ALERT>\n",
                outdated_warnings.join("\n"),
                recommended_replacement.as_deref().unwrap_or("Frontier 2026 models (Opus 5.5, Sonnet 5.5, Grok 4.7, Astra 6, Sol 6.1)")
            )
        } else {
            String::new()
        };

        let calibration_block = format!(
            "<chronofact_temporal_anchor>\n\
            [SYSTEM TEMPORAL CALIBRATION ANCHOR]\n\
            CURRENT_EVALUATION_DATE: {}\n\
            SELECTED_MODEL: {} ({})\n\
            MODEL_STATUS: {}\n\
            OFFICIAL_KNOWLEDGE_CUTOFF: {} (Delta: +{} days)\n\
            ESTIMATED_TRAINING_FREEZE: {} (Delta: +{} days)\n\
            QUERY_TEMPORAL_RISK: {:.2}\n\
            GROUNDING_MANDATORY: {}{}\n\
            INSTRUCTION: Your internal neural weights are static and permanently frozen at the training freeze date.\n\
            Any events, tool updates, model releases, library APIs, or breaking changes after that date\n\
            do NOT exist in your weights. You MUST rely on supplied grounded research and explicitly state\n\
            knowledge boundaries rather than hallucinating plausible details.\n\
            </chronofact_temporal_anchor>",
            current_date.format("%Y-%m-%d"),
            model.display_name,
            model.vendor,
            model.status,
            model.official_knowledge_cutoff.format("%Y-%m-%d"),
            days_post_cutoff,
            model.estimated_training_freeze.format("%Y-%m-%d"),
            days_post_freeze,
            scan.temporal_risk_score,
            if requires_grounding { "YES" } else { "NO" },
            warning_section
        );

        HorizonAnalysis {
            current_date,
            model_id: model.model_id.clone(),
            model_name: model.display_name.clone(),
            model_status: model.status.clone(),
            official_cutoff: model.official_knowledge_cutoff,
            training_freeze: model.estimated_training_freeze,
            days_post_cutoff,
            days_post_freeze,
            temporal_risk_score: scan.temporal_risk_score,
            is_model_outdated_or_retired: is_retired,
            recommended_replacement,
            outdated_warnings,
            requires_grounding,
            calibration_block,
        }
    }
}
