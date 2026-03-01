use std::collections::BTreeSet;

use crate::domain::{
    AssistantIntent, NarrativeCharacter, NarrativeEvent, NarrativeNudge, NarrativeSnapshot,
    WorkingMemory,
};
use crate::ports::{
    AssistantAgent, AssistantResponse, CharacterParser, EventParser, FollowUpDirective,
    NudgeGenerator,
};
use uuid::Uuid;

#[derive(Default)]
pub struct StubNarrativeEngine;

impl AssistantAgent for StubNarrativeEngine {
    fn interpret_followup(
        &self,
        _prompt: &str,
        _memory: &WorkingMemory,
    ) -> Result<FollowUpDirective, String> {
        Ok(FollowUpDirective::Unknown)
    }

    fn elaborate_focus(
        &self,
        _prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let focus = memory
            .current_focus
            .as_ref()
            .map(|focus| focus.summary.as_str())
            .unwrap_or("the current idea");
        Ok(AssistantResponse::FinalReply {
            intent: AssistantIntent::Guide,
            title: "Idea".to_string(),
            focus_summary: Some(focus.to_string()),
            body: format!(
                "Here is a deeper take on {}.\nDevelop its atmosphere, social texture, and the conflict it creates for the story.",
                focus
            ),
        })
    }

    fn suggest_alternative(
        &self,
        _prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let topic = format!("{:?}", memory.conversation_topic).to_ascii_lowercase();
        Ok(AssistantResponse::FinalReply {
            intent: AssistantIntent::Guide,
            title: "Idea".to_string(),
            focus_summary: Some(format!("Alternative {} direction", topic)),
            body: format!(
                "Here is another {} direction.\nPick one concrete detail and I will build it out further.",
                topic
            ),
        })
    }

    fn brainstorm_topic(
        &self,
        _prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let topic = format!("{:?}", memory.conversation_topic).to_ascii_lowercase();
        Ok(AssistantResponse::FinalReply {
            intent: AssistantIntent::Guide,
            title: "Idea".to_string(),
            focus_summary: Some(format!("{} direction", topic)),
            body: format!(
                "Here is one {} direction to explore.\nGive me one concrete aspect and I will develop it further.",
                topic
            ),
        })
    }

    fn draft_from_focus(
        &self,
        _prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let focus = memory
            .current_focus
            .as_ref()
            .map(|focus| focus.summary.as_str())
            .unwrap_or("the current idea");
        Ok(AssistantResponse::FinalReply {
            intent: AssistantIntent::MutateDocument,
            title: "Screenplay Draft".to_string(),
            focus_summary: Some(focus.to_string()),
            body: format!("A scene grows out of {}.", focus),
        })
    }

    fn generate_nudge(
        &self,
        snapshot: &NarrativeSnapshot,
        _memory: &WorkingMemory,
    ) -> Result<NarrativeNudge, String> {
        NudgeGenerator::generate_nudge(self, snapshot)
    }
}

impl CharacterParser for StubNarrativeEngine {
    fn parse_character(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeCharacter, String> {
        let cleaned = normalize_whitespace(description);
        if cleaned.is_empty() {
            return Err("character description is empty".to_string());
        }

        let name = infer_character_name(&cleaned)
            .unwrap_or_else(|| fallback_character_name(snapshot.characters.len()));
        let tags = infer_character_tags(&cleaned);

        Ok(NarrativeCharacter {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            name,
            summary: cleaned,
            tags,
        })
    }
}

impl EventParser for StubNarrativeEngine {
    fn parse_event(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeEvent, String> {
        let cleaned = normalize_whitespace(description);
        if cleaned.is_empty() {
            return Err("event description is empty".to_string());
        }

        Ok(NarrativeEvent {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            title: infer_event_title(&cleaned),
            summary: cleaned.clone(),
            participants: infer_event_participants(&cleaned, snapshot),
        })
    }
}

impl NudgeGenerator for StubNarrativeEngine {
    fn generate_nudge(&self, snapshot: &NarrativeSnapshot) -> Result<NarrativeNudge, String> {
        // In a real agent loop, the agent would analyze the snapshot and backlog.
        // For the stub, we look for obvious gaps first.
        let message = if snapshot.characters.is_empty() {
            "Start by introducing your protagonist. What is their core desire?".to_string()
        } else if snapshot.events.is_empty() {
            "Describe an initial event that challenges your protagonist's world.".to_string()
        } else if snapshot.characters.len() == 1 {
            "Every hero needs a counter-force. Who opposes or challenges them?".to_string()
        } else if snapshot.events.iter().all(|event| event.participants.is_empty()) {
            "Which characters are present in your key events? Linking them helps the model reason.".to_string()
        } else {
            "What's the next logical step in the story? A new character, a turning point, or a relationship shift?".to_string()
        };

        Ok(NarrativeNudge { message })
    }
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn infer_character_name(description: &str) -> Option<String> {
    let named = ["named ", "called ", "is ", "introduce "];
    for marker in named {
        if let Some(index) = description.to_lowercase().find(marker) {
            let suffix = description.get(index + marker.len()..)?.trim();
            if let Some(candidate) = leading_name_phrase(suffix) {
                return Some(candidate);
            }
        }
    }

    leading_name_phrase(description)
}

fn leading_name_phrase(value: &str) -> Option<String> {
    let mut words = Vec::new();

    for token in value.split_whitespace() {
        let cleaned = token
            .trim_matches(|character: char| !character.is_alphanumeric() && character != '\'' && character != '-');
        if cleaned.is_empty() {
            continue;
        }

        let starts_uppercase = cleaned
            .chars()
            .next()
            .map(|character| character.is_ascii_uppercase())
            .unwrap_or(false);

        if starts_uppercase {
            words.push(cleaned.to_string());
            if words.len() == 3 {
                break;
            }
        } else if !words.is_empty() {
            break;
        }
    }

    if words.is_empty() {
        None
    } else {
        Some(words.join(" "))
    }
}

fn fallback_character_name(existing_character_count: usize) -> String {
    format!("Character {}", existing_character_count + 1)
}

fn infer_character_tags(description: &str) -> Vec<String> {
    let lowercase = description.to_lowercase();
    let mut tags = BTreeSet::new();

    let keyword_tags = [
        ("prince", "royalty"),
        ("queen", "royalty"),
        ("king", "royalty"),
        ("advisor", "politics"),
        ("general", "military"),
        ("war", "conflict"),
        ("prophecy", "mysticism"),
        ("temple", "faith"),
        ("rival", "opposition"),
        ("mentor", "guidance"),
        ("friend", "ally"),
        ("villain", "antagonist"),
        ("lead", "protagonist"),
        ("protagonist", "protagonist"),
    ];

    for (keyword, tag) in keyword_tags {
        if lowercase.contains(keyword) {
            tags.insert(tag.to_string());
        }
    }

    if tags.is_empty() {
        tags.insert("character".to_string());
        tags.insert("draft".to_string());
    }

    tags.into_iter().collect()
}

fn infer_event_title(description: &str) -> String {
    let title = description
        .split(&['.', '!', '?'][..])
        .next()
        .unwrap_or(description)
        .split_whitespace()
        .filter_map(|token| {
            let cleaned = token.trim_matches(|character: char| !character.is_alphanumeric() && character != '\'' && character != '-');
            (!cleaned.is_empty()).then_some(cleaned)
        })
        .take(6)
        .map(to_title_case)
        .collect::<Vec<_>>()
        .join(" ");

    if title.is_empty() {
        "Narrative Event".to_string()
    } else {
        title
    }
}

fn to_title_case(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => {
            let mut result = first.to_ascii_uppercase().to_string();
            result.push_str(&characters.as_str().to_ascii_lowercase());
            result
        }
        None => String::new(),
    }
}

fn infer_event_participants(description: &str, snapshot: &NarrativeSnapshot) -> Vec<Uuid> {
    let lowercase = description.to_lowercase();
    snapshot
        .characters
        .iter()
        .filter(|character| lowercase.contains(&character.name.to_lowercase()))
        .map(|character| character.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_character_name_and_tags() {
        let engine = StubNarrativeEngine;
        let snapshot = NarrativeSnapshot::default();
        let character = engine
            .parse_character(
                "The lead is Prince Arjun, a conflicted heir avoiding war.",
                &snapshot,
            )
            .unwrap();

        assert_eq!(character.name, "Prince Arjun");
        assert!(character.tags.iter().any(|tag| tag == "royalty"));
        assert!(character.tags.iter().any(|tag| tag == "conflict"));
    }

    #[test]
    fn links_known_characters_into_events() {
        let engine = StubNarrativeEngine;
        let character = NarrativeCharacter {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            name: "Arjun".to_string(),
            summary: "Lead".to_string(),
            tags: vec!["protagonist".to_string()],
        };
        let snapshot = NarrativeSnapshot {
            characters: vec![character.clone()],
            ..Default::default()
        };

        let event = engine
            .parse_event("Arjun confronts the royal council at dawn.", &snapshot)
            .unwrap();

        assert!(event.participants.contains(&character.id));
    }
}
