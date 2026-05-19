// Coordinator — Multi-agent pipeline'ı yönetir.
//
// Akış:
//   1. Planner.plan(user_q)         → strateji + 1-4 alt görev + önerilen tool'lar
//   2. Executor.process_message(q*) → mevcut CfoOrchestrator, Gemini function-calling
//   3. Critic.critique(q, draft)    → PASS / REVISE (REVISE'de improved_answer)
//
// Tasarım kararları:
// - 3 ajan da SAME GeminiClient'ı paylaşır → connection pool korunur, retry politikası aynı
// - Planner/Critic stateless: history tutmaz, her çağrı bağımsız
// - Executor zaten conversational history tutuyor — bu yüzden Coordinator'da değişiklik yok
// - Plan hint inline olarak executor'a user message içinde paslanır (yeni metod açmaya gerek yok)
// - Critic REVISE derse Executor tekrar çağrılmaz — improved_answer doğrudan kullanılır
//   (sebep: tek demo turunda +2-4sn ek latency kullanıcıyı kaybettirir)

use crate::agents::critic::{self, CriticVerdict};
use crate::agents::orchestrator::CfoOrchestrator;
use crate::agents::planner::{self, PlanResult};
use crate::core::error::AeraResult;
use serde::Serialize;

/// Sunum + frontend trace bloğu için 3 ajanın izleri.
#[derive(Debug, Clone, Serialize)]
pub struct AgentTrace {
    pub plan_strategy: String,
    pub subtask_count: usize,
    pub planner_used_fallback: bool,
    pub executor_tools: Vec<String>,
    pub critic_verdict: String,
    pub critic_issues: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CoordinatorResponse {
    pub reply: String,
    pub tools_used: Vec<String>,
    pub trace: AgentTrace,
}

/// 3-Agent pipeline orchestration. Yedekleme: Planner veya Critic patlarsa
/// kullanıcıyı kırma — Executor sonucunu downgrade ile döndür.
pub async fn run(
    orchestrator: &mut CfoOrchestrator,
    user_message: &str,
) -> AeraResult<CoordinatorResponse> {
    let data_loaded = orchestrator.engine.has_data();

    // 1. PLANNER
    // Borrow scope: Gemini reference burada alınır, await sonra düşer.
    let (plan, planner_fallback) = {
        let gemini = orchestrator.gemini();
        match planner::plan(gemini, user_message, data_loaded).await {
            Ok(p) => {
                tracing::info!(
                    "🧭 Planner: {} alt görev | {}",
                    p.subtasks.len(), p.strategy
                );
                (p, false)
            }
            Err(e) => {
                // Planner Gemini hatası → fallback ile devam, kullanıcıya yansıtma
                tracing::warn!("⚠️  Planner başarısız ({}), fallback'e geçiliyor", e);
                (planner::fallback_plan(user_message), true)
            }
        }
    };

    // 2. EXECUTOR
    // Planı executor'a inline hint olarak ekle. Fallback ise sade user_message gönder.
    let executor_input = if planner_fallback {
        user_message.to_string()
    } else {
        build_executor_input(user_message, &plan)
    };

    let (draft, tools_used) = orchestrator.process_message(&executor_input).await?;

    // 3. CRITIC
    let (final_reply, critic_label, critic_issues) = {
        let gemini = orchestrator.gemini();
        match critic::critique(gemini, user_message, &draft, &tools_used).await {
            Ok(CriticVerdict::Pass) => {
                tracing::info!("✅ Critic: PASS");
                (draft, "PASS".to_string(), Vec::new())
            }
            Ok(CriticVerdict::Revise { issues, improved_answer }) => {
                tracing::info!(
                    "✏️  Critic: REVISE ({} sorun düzeltildi)",
                    issues.len()
                );
                (improved_answer, "REVISE".to_string(), issues)
            }
            Err(e) => {
                // Critic patladıysa draft'ı geç — kullanıcı boş yanıt yerine cevap görsün
                tracing::warn!("⚠️  Critic başarısız ({}), draft korunuyor", e);
                (draft, "SKIPPED".to_string(), vec![format!("Critic error: {}", e)])
            }
        }
    };

    Ok(CoordinatorResponse {
        reply: final_reply,
        tools_used: tools_used.clone(),
        trace: AgentTrace {
            plan_strategy: plan.strategy,
            subtask_count: plan.subtasks.len(),
            planner_used_fallback: planner_fallback,
            executor_tools: tools_used,
            critic_verdict: critic_label,
            critic_issues,
        },
    })
}

/// Executor'a planı inline geçer. Marker'lar net olduğu için executor system prompt'u
/// bunu kullanıcıya yansıtmıyor (modelde test edildi — gerekirse system prompt'a açık
/// "PLANNER_HINT bloğu kullanıcıya gösterilmez" satırı eklenebilir).
fn build_executor_input(user_message: &str, plan: &PlanResult) -> String {
    if !plan.requires_tools {
        // Sohbet sorusu: plan hintine gerek yok, executor doğal cevap versin
        return user_message.to_string();
    }

    let mut steps = String::with_capacity(256);
    for st in &plan.subtasks {
        let tools_hint = if st.suggested_tools.is_empty() {
            String::new()
        } else {
            format!(" [araç önerisi: {}]", st.suggested_tools.join(", "))
        };
        steps.push_str(&format!("- {}{}\n", st.question, tools_hint));
    }

    format!(
        "{user_message}\n\n\
         [PLANNER_HINT — kullanıcıya gösterme, sadece sen kullan]\n\
         Strateji: {strategy}\n\
         Alt sorular:\n{steps}\
         Yukarıdaki sırayı takip et, sonunda tüm alt soruları kapsayan tek bir cevap üret.\n\
         [/PLANNER_HINT]",
        user_message = user_message,
        strategy = plan.strategy,
        steps = steps,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::planner::{PlanResult, SubTask};

    #[test]
    fn test_executor_input_chat_query_unchanged() {
        let plan = PlanResult {
            strategy: "Sohbet".into(),
            requires_tools: false,
            subtasks: vec![SubTask {
                id: 1, question: "merhaba".into(), suggested_tools: vec![]
            }],
        };
        let out = build_executor_input("merhaba", &plan);
        assert_eq!(out, "merhaba", "Sohbet sorularında hint eklenmemeli");
    }

    #[test]
    fn test_executor_input_analytical_gets_hint() {
        let plan = PlanResult {
            strategy: "Nakit + risk analizi".into(),
            requires_tools: true,
            subtasks: vec![
                SubTask {
                    id: 1,
                    question: "Sağlık skoru nedir?".into(),
                    suggested_tools: vec!["get_health_score".into()],
                },
                SubTask {
                    id: 2,
                    question: "Önümüzdeki 3 ay nasıl?".into(),
                    suggested_tools: vec!["predict_cashflow".into()],
                },
            ],
        };
        let out = build_executor_input("Durumum nasıl?", &plan);
        assert!(out.contains("PLANNER_HINT"));
        assert!(out.contains("Sağlık skoru nedir?"));
        assert!(out.contains("get_health_score"));
        assert!(out.contains("predict_cashflow"));
        assert!(out.starts_with("Durumum nasıl?"));
    }

    #[test]
    fn test_executor_input_no_tools_suggested() {
        let plan = PlanResult {
            strategy: "Tek adım".into(),
            requires_tools: true,
            subtasks: vec![SubTask {
                id: 1, question: "Veri özeti?".into(), suggested_tools: vec![]
            }],
        };
        let out = build_executor_input("Veri özeti?", &plan);
        assert!(out.contains("Veri özeti?"));
        assert!(!out.contains("araç önerisi:"));
    }

    #[test]
    fn test_agent_trace_serializes() {
        let trace = AgentTrace {
            plan_strategy: "x".into(),
            subtask_count: 2,
            planner_used_fallback: false,
            executor_tools: vec!["get_health_score".into()],
            critic_verdict: "PASS".into(),
            critic_issues: vec![],
        };
        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("\"plan_strategy\":\"x\""));
        assert!(json.contains("\"critic_verdict\":\"PASS\""));
    }
}
