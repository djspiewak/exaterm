use serde::{Deserialize, Serialize};

/// Per-field character budget computed from a card's pixel width at layout time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardCharBudget {
    pub title_chars: u16,
    pub headline_chars: u16,
    pub detail_chars: u16,
    pub alert_chars: u16,
}

impl CardCharBudget {
    /// Pessimistic defaults used before the first `ReportCardBudget` message
    /// arrives from a client. Matches roughly a 240 px wide card — suitable for
    /// very narrow layouts and ensures the AI is never told it has unlimited space.
    pub const DEFAULT_WORST_CASE: Self = Self {
        title_chars: 24,
        headline_chars: 40,
        detail_chars: 40,
        alert_chars: 35,
    };
}

/// Truncate `text` to at most `max_chars` Unicode scalar values, preferring a
/// word boundary when truncation is needed, and appending `…` on cut.
pub fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    let bounded = truncated
        .rfind(char::is_whitespace)
        .map(|idx| truncated[..idx].trim_end())
        .filter(|s| !s.is_empty())
        .unwrap_or(&truncated);
    format!("{bounded}…")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TacticalState {
    Idle,
    Stopped,
    Thinking,
    Working,
    Blocked,
    Failed,
    Complete,
    Detached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionLevel {
    Autopilot,
    Monitor,
    Guide,
    Intervene,
    Takeover,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TacticalSynthesis {
    pub tactical_state: TacticalState,
    pub tactical_state_brief: Option<String>,
    pub attention_level: AttentionLevel,
    pub attention_brief: Option<String>,
    pub headline: Option<String>,
}

impl TacticalSynthesis {
    pub fn sanitize(mut self, limits: Option<&CardCharBudget>) -> Self {
        self.headline = sanitize_optional(self.headline);
        self.tactical_state_brief = sanitize_optional(self.tactical_state_brief);
        self.attention_brief = sanitize_optional(self.attention_brief);
        if let Some(limits) = limits {
            self.headline = self
                .headline
                .map(|s| truncate_with_ellipsis(&s, limits.headline_chars.into()));
            self.tactical_state_brief = self
                .tactical_state_brief
                .map(|s| truncate_with_ellipsis(&s, limits.detail_chars.into()));
            self.attention_brief = self
                .attention_brief
                .map(|s| truncate_with_ellipsis(&s, limits.alert_chars.into()));
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameSuggestion {
    pub name: String,
}

impl NameSuggestion {
    pub fn sanitize(mut self, max_chars: Option<u16>) -> Self {
        self.name = sanitize_name(&self.name, max_chars.unwrap_or(40).into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NudgeSuggestion {
    pub text: String,
}

impl NudgeSuggestion {
    pub fn sanitize(mut self) -> Self {
        self.text = sanitize_optional(Some(self.text))
            .unwrap_or_default()
            .chars()
            .take(120)
            .collect();
        self
    }
}

fn sanitize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        (!text.is_empty()).then_some(text)
    })
}

fn sanitize_name(value: &str, max_chars: usize) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let truncated = trimmed.chars().take(max_chars).collect::<String>();
    let bounded = if truncated.chars().count() < trimmed.chars().count() {
        truncated
            .rfind(char::is_whitespace)
            .map(|index| truncated[..index].trim_end().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(truncated)
    } else {
        truncated
    };

    bounded
        .trim()
        .trim_matches(|ch: char| matches!(ch, '"' | '\'' | '.' | ',' | ':' | ';'))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        truncate_with_ellipsis, AttentionLevel, CardCharBudget, NameSuggestion, NudgeSuggestion,
        TacticalState, TacticalSynthesis,
    };

    #[test]
    fn tactical_synthesis_sanitize_trims_fields() {
        let summary = TacticalSynthesis {
            tactical_state: TacticalState::Stopped,
            tactical_state_brief: Some("  stopped   cleanly  ".into()),
            attention_level: AttentionLevel::Guide,
            attention_brief: Some("  likely needs   a small nudge  ".into()),
            headline: Some("  parser   pass ".into()),
        }
        .sanitize(None);

        assert_eq!(summary.headline.as_deref(), Some("parser pass"));
        assert_eq!(
            summary.tactical_state_brief.as_deref(),
            Some("stopped cleanly")
        );
        assert_eq!(
            summary.attention_brief.as_deref(),
            Some("likely needs a small nudge")
        );
    }

    #[test]
    fn tactical_synthesis_sanitize_truncates_with_budget() {
        let budget = CardCharBudget {
            title_chars: 10,
            headline_chars: 15,
            detail_chars: 12,
            alert_chars: 10,
        };
        let summary = TacticalSynthesis {
            tactical_state: TacticalState::Working,
            tactical_state_brief: Some("compiling the full project tree right now".into()),
            attention_level: AttentionLevel::Autopilot,
            attention_brief: Some("routine build loop no action needed".into()),
            headline: Some("building all workspace crates simultaneously".into()),
        }
        .sanitize(Some(&budget));

        let headline = summary.headline.as_deref().unwrap();
        assert!(headline.chars().count() <= budget.headline_chars as usize);
        assert!(headline.ends_with('…'));

        let brief = summary.tactical_state_brief.as_deref().unwrap();
        assert!(brief.chars().count() <= budget.detail_chars as usize);
        assert!(brief.ends_with('…'));

        let alert = summary.attention_brief.as_deref().unwrap();
        assert!(alert.chars().count() <= budget.alert_chars as usize);
        assert!(alert.ends_with('…'));
    }

    #[test]
    fn tactical_synthesis_sanitize_no_ellipsis_when_under_budget() {
        let budget = CardCharBudget {
            title_chars: 100,
            headline_chars: 100,
            detail_chars: 100,
            alert_chars: 100,
        };
        let summary = TacticalSynthesis {
            tactical_state: TacticalState::Working,
            tactical_state_brief: Some("short brief".into()),
            attention_level: AttentionLevel::Autopilot,
            attention_brief: Some("no action".into()),
            headline: Some("short headline".into()),
        }
        .sanitize(Some(&budget));

        assert_eq!(summary.headline.as_deref(), Some("short headline"));
        assert_eq!(summary.tactical_state_brief.as_deref(), Some("short brief"));
        assert_eq!(summary.attention_brief.as_deref(), Some("no action"));
    }

    #[test]
    fn name_suggestion_sanitize_bounds_length() {
        let suggestion = NameSuggestion {
            name: "  a very long parser repair name that should definitely be shortened  ".into(),
        }
        .sanitize(None);
        assert!(suggestion.name.len() <= 40);
        assert!(!suggestion.name.is_empty());
    }

    #[test]
    fn name_suggestion_sanitize_custom_max() {
        let suggestion = NameSuggestion {
            name: "fix parser edge case handling in the type checker".into(),
        }
        .sanitize(Some(25));
        assert!(suggestion.name.chars().count() <= 25);
        assert!(!suggestion.name.is_empty());
    }

    #[test]
    fn nudge_suggestion_sanitize_trims() {
        let suggestion = NudgeSuggestion {
            text: "   Keep going on the next concrete failure.   ".into(),
        }
        .sanitize();
        assert_eq!(suggestion.text, "Keep going on the next concrete failure.");
    }

    #[test]
    fn truncate_with_ellipsis_zero_max_returns_empty() {
        assert_eq!(truncate_with_ellipsis("hello", 0), "");
    }

    #[test]
    fn truncate_with_ellipsis_no_cut_when_under_limit() {
        assert_eq!(truncate_with_ellipsis("hello world", 20), "hello world");
    }

    #[test]
    fn truncate_with_ellipsis_cuts_at_word_boundary() {
        let result = truncate_with_ellipsis("building all workspace crates", 20);
        assert!(result.ends_with('…'));
        assert!(result.chars().count() <= 20);
        // Should not cut mid-word — no partial word fragment should appear.
        let without_ellipsis = result.trim_end_matches('…');
        assert!(!without_ellipsis.ends_with("worksp"), "cut mid-word: {result}");
    }

    #[test]
    fn truncate_with_ellipsis_falls_back_to_hard_cut_when_no_space() {
        let result = truncate_with_ellipsis("averylongwordwithoutspaces", 10);
        assert!(result.ends_with('…'));
        assert!(result.chars().count() <= 10);
    }
}
