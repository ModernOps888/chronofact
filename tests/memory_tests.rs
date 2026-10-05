use chronofact::{MemoryEngine, ProjectEntity, SessionSummary};

#[test]
fn test_epistemic_memory_cross_session_retention() {
    let memory = MemoryEngine::new_in_memory().expect("failed to init db");

    let project_id = "test-project-alpha";

    // 1. Session 1: Define database and tech stack
    let session_1 = SessionSummary {
        session_id: "chat-turn-001".to_string(),
        project_id: project_id.to_string(),
        created_at: "2026-10-01T10:00:00Z".to_string(),
        summary: "Established project foundation and tech stack".to_string(),
    };
    memory.record_session(&session_1).unwrap();

    let entity1 = ProjectEntity {
        id: format!("{}:react", project_id),
        project_id: project_id.to_string(),
        entity_name: "React".to_string(),
        entity_type: "TECH_STACK".to_string(),
        definition: "Frontend framework, strictly React 19 with Server Components".to_string(),
        version: "19.2.0".to_string(),
        updated_at: "2026-10-01T10:05:00Z".to_string(),
    };
    memory.upsert_entity(&entity1).unwrap();

    let entity2 = ProjectEntity {
        id: format!("{}:auth_rule", project_id),
        project_id: project_id.to_string(),
        entity_name: "JWT Authentication Invariant".to_string(),
        entity_type: "ARCHITECTURE_RULE".to_string(),
        definition: "All external API endpoints must enforce Ed25519 token signatures".to_string(),
        version: "v1".to_string(),
        updated_at: "2026-10-01T10:10:00Z".to_string(),
    };
    memory.upsert_entity(&entity2).unwrap();

    // Record an architectural decision event in session 1
    memory.record_event(
        "chat-turn-001",
        "DECISION",
        "Decided to reject OAuth2 password grant in favor of PKCE flow",
        None,
    ).unwrap();

    // 2. Fast forward to Session 5 (5 chats later) in the same project
    let session_5 = SessionSummary {
        session_id: "chat-turn-005".to_string(),
        project_id: project_id.to_string(),
        created_at: "2026-10-05T10:00:00Z".to_string(),
        summary: "Building user profile service".to_string(),
    };
    memory.record_session(&session_5).unwrap();

    // 3. Compile Project Truth Dossier for session 5
    let dossier = memory.get_dossier(project_id).unwrap();

    // Verify: Session 5 remembers everything from session 1 without amnesia!
    assert!(dossier.active_tech_stack.iter().any(|s| s.contains("React 19")));
    assert!(dossier.architectural_invariants.iter().any(|s| s.contains("Ed25519")));
    assert!(dossier.recent_decisions.iter().any(|s| s.contains("PKCE flow")));
    assert!(dossier.dossier_markdown.contains("<chronofact_project_truth_dossier"));
}

#[test]
fn test_relevance_gated_memory_prevents_context_pollution() {
    let memory = MemoryEngine::new_in_memory().expect("failed to init db");
    let project_id = "test-project-beta";

    // Setup an entity for React and an entity for authentication
    let entity1 = ProjectEntity {
        id: format!("{}:react", project_id),
        project_id: project_id.to_string(),
        entity_name: "React 19 Framework".to_string(),
        entity_type: "TECH_STACK".to_string(),
        definition: "Frontend strictly built with React 19.2 Server Components".to_string(),
        version: "19.2".to_string(),
        updated_at: "2026-10-01T10:00:00Z".to_string(),
    };
    memory.upsert_entity(&entity1).unwrap();

    let entity2 = ProjectEntity {
        id: format!("{}:auth", project_id),
        project_id: project_id.to_string(),
        entity_name: "API Authentication Rule".to_string(),
        entity_type: "ARCHITECTURE_RULE".to_string(),
        definition: "All external API endpoints must enforce Ed25519 token signatures".to_string(),
        version: "v1".to_string(),
        updated_at: "2026-10-01T10:00:00Z".to_string(),
    };
    memory.upsert_entity(&entity2).unwrap();

    // 1. Unrelated query: e.g. algorithm, general math, or new topic
    // Zero context pollution: Dossier MUST BE completely empty string!
    let unrelated = memory.get_relevant_dossier(project_id, "Explain the time complexity of quicksort algorithm").unwrap();
    assert!(unrelated.dossier_markdown.is_empty(), "Unrelated query must not receive dumped memory");
    assert!(unrelated.active_tech_stack.is_empty());
    assert!(unrelated.architectural_invariants.is_empty());

    // 2. Specific query about authentication
    // Only the relevant invariant is returned; unrelated React memory is NOT vomited
    let auth_query = memory.get_relevant_dossier(project_id, "How do we handle API authentication tokens?").unwrap();
    assert!(!auth_query.dossier_markdown.is_empty());
    assert_eq!(auth_query.architectural_invariants.len(), 1);
    assert!(auth_query.architectural_invariants[0].contains("Ed25519"));
    assert!(auth_query.active_tech_stack.is_empty(), "Must not inject unrelated React stack when asking about auth");
}

