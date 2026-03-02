use crate::domain::{
    NarrativeChangeKind, NarrativeChangeSummary, NarrativeCharacter, NarrativeCommitTarget,
    NarrativeEvent, NarrativeMessagePreview, NarrativeSnapshot,
    OntologyRelationship, OntologyRelationshipKind,
};
use crate::ports::{CharacterParser, EventParser};

pub struct NarrativeService<C, E> {
    character_parser: C,
    event_parser: E,
}

impl<C, E> NarrativeService<C, E> {
    pub fn new(character_parser: C, event_parser: E) -> Self {
        Self {
            character_parser,
            event_parser,
        }
    }
}

impl<C, E> NarrativeService<C, E>
where
    C: CharacterParser,
    E: EventParser,
{
    pub fn parse_character(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeCharacter, String> {
        self.character_parser.parse_character(description, snapshot)
    }

    pub fn parse_event(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeEvent, String> {
        self.event_parser.parse_event(description, snapshot)
    }

    pub fn preview_message(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeMessagePreview, String> {
        let prompt = description.trim().to_string();
        if prompt.is_empty() {
            return Ok(NarrativeMessagePreview {
                prompt,
                suggested_target: NarrativeCommitTarget::Character,
                character: None,
                event: None,
                relationships: Vec::new(),
                changes: Vec::new(),
                reply_title: None,
                reply_body: None,
            });
        }

        let relationship_statement = extract_relationship_statement(&prompt);
        let character = self
            .parse_character(&prompt, snapshot)
            .ok()
            .map(|character| merge_character(character, snapshot));
        let event = if relationship_statement.is_some() {
            None
        } else {
            self.parse_event(&prompt, snapshot)
                .ok()
                .map(|event| merge_event(event, snapshot))
        };
        let suggested_target = suggested_target(&prompt, relationship_statement.is_some());
        let relationships = infer_relationships(&prompt, snapshot, relationship_statement.as_ref());
        let changes = build_change_summaries(
            &character,
            &event,
            &relationships,
            suggested_target.clone(),
        );

        Ok(NarrativeMessagePreview {
            prompt,
            suggested_target,
            character,
            event,
            relationships,
            changes,
            reply_title: None,
            reply_body: None,
        })
    }
}

fn merge_character(
    mut parsed: NarrativeCharacter,
    snapshot: &NarrativeSnapshot,
) -> NarrativeCharacter {
    if let Some(existing) = snapshot
        .characters
        .iter()
        .find(|existing| existing.name.eq_ignore_ascii_case(&parsed.name))
    {
        parsed.id = existing.id;
        parsed.ontology_entity_id = existing.ontology_entity_id;
    }

    parsed
}

fn merge_event(mut parsed: NarrativeEvent, snapshot: &NarrativeSnapshot) -> NarrativeEvent {
    if let Some(existing) = snapshot
        .events
        .iter()
        .find(|existing| existing.title.eq_ignore_ascii_case(&parsed.title))
    {
        parsed.id = existing.id;
        parsed.ontology_entity_id = existing.ontology_entity_id;
    }

    parsed
}

fn build_change_summaries(
    character: &Option<NarrativeCharacter>,
    event: &Option<NarrativeEvent>,
    relationships: &[OntologyRelationship],
    suggested_target: NarrativeCommitTarget,
) -> Vec<NarrativeChangeSummary> {
    let mut changes = Vec::new();

    if let Some(character) = character {
        let kind = if character.ontology_entity_id.is_some() {
            NarrativeChangeKind::UpdateCharacter
        } else {
            NarrativeChangeKind::AddCharacter
        };
        changes.push(NarrativeChangeSummary {
            kind,
            label: character.name.clone(),
            detail: character.summary.clone(),
        });
    }

    if let Some(event) = event {
        let kind = if event.ontology_entity_id.is_some() {
            NarrativeChangeKind::UpdateEvent
        } else {
            NarrativeChangeKind::AddEvent
        };
        let detail = if event.participants.is_empty() {
            event.summary.clone()
        } else {
            format!(
                "{} Participants tracked: {}.",
                event.summary,
                event.participants.len()
            )
        };
        changes.push(NarrativeChangeSummary {
            kind,
            label: event.title.clone(),
            detail,
        });
    }

    for relationship in relationships {
        let kind = if relationship.summary.starts_with("Update ") {
            NarrativeChangeKind::UpdateRelationship
        } else {
            NarrativeChangeKind::AddRelationship
        };
        changes.push(NarrativeChangeSummary {
            kind,
            label: relationship_label(relationship),
            detail: relationship.summary.clone(),
        });
    }

    changes.sort_by_key(|change| match (&suggested_target, &change.kind) {
        (NarrativeCommitTarget::Character, NarrativeChangeKind::AddCharacter)
        | (NarrativeCommitTarget::Character, NarrativeChangeKind::UpdateCharacter)
        | (NarrativeCommitTarget::Event, NarrativeChangeKind::AddEvent)
        | (NarrativeCommitTarget::Event, NarrativeChangeKind::UpdateEvent) => 0,
        _ => 1,
    });

    changes
}

fn infer_relationships(
    prompt: &str,
    snapshot: &NarrativeSnapshot,
    relationship_statement: Option<&RelationshipStatement>,
) -> Vec<OntologyRelationship> {
    let lowered = prompt.to_lowercase();
    let Some(kind) = infer_relationship_kind(&lowered, relationship_statement) else {
        return Vec::new();
    };

    let matched = if let Some(statement) = relationship_statement {
        snapshot
            .characters
            .iter()
            .filter(|character| {
                character.name.eq_ignore_ascii_case(&statement.subject)
                    || character.name.eq_ignore_ascii_case(&statement.object)
            })
            .collect::<Vec<_>>()
    } else {
        snapshot
            .characters
            .iter()
            .filter(|character| lowered.contains(&character.name.to_lowercase()))
            .collect::<Vec<_>>()
    };

    if matched.len() < 2 {
        return Vec::new();
    }

    let (source, target) = if let Some(statement) = relationship_statement {
        let source = matched
            .iter()
            .copied()
            .find(|character| character.name.eq_ignore_ascii_case(&statement.subject));
        let target = matched
            .iter()
            .copied()
            .find(|character| character.name.eq_ignore_ascii_case(&statement.object));

        match (source, target) {
            (Some(source), Some(target)) => (source, target),
            _ => return Vec::new(),
        }
    } else {
        (matched[0], matched[1])
    };
    let (Some(source_id), Some(target_id)) = (source.ontology_entity_id, target.ontology_entity_id) else {
        return Vec::new();
    };

    let existing = snapshot.ontology_graph.relationships.iter().find(|relationship| {
        relationship.source_id == source_id
            && relationship.target_id == target_id
            && relationship.kind == kind
    });
    let summary = match existing {
        Some(_) => format!("Update {}.", relationship_summary(&source.name, &target.name, &kind)),
        None => relationship_summary(&source.name, &target.name, &kind),
    };

    vec![OntologyRelationship {
        id: existing.map(|relationship| relationship.id).unwrap_or_else(uuid::Uuid::new_v4),
        source_id: source_id,
        target_id: target_id,
        kind,
        summary,
    }]
}

fn infer_relationship_kind(
    prompt: &str,
    relationship_statement: Option<&RelationshipStatement>,
) -> Option<OntologyRelationshipKind> {
    if relationship_statement
        .map(|statement| matches!(statement.kind, RelationshipStatementKind::Sibling))
        .unwrap_or(false)
    {
        Some(OntologyRelationshipKind::SiblingOfCharacter)
    } else if ["opposes", "against", "enemy", "rival", "conflicts with"]
        .iter()
        .any(|marker| prompt.contains(marker))
    {
        Some(OntologyRelationshipKind::OpposesCharacter)
    } else if ["advises", "mentor", "counsels", "guides", "advisor to"]
        .iter()
        .any(|marker| prompt.contains(marker))
    {
        Some(OntologyRelationshipKind::AdvisesCharacter)
    } else if ["supports", "helps", "backs", "ally of", "stands with"]
        .iter()
        .any(|marker| prompt.contains(marker))
    {
        Some(OntologyRelationshipKind::SupportsCharacter)
    } else {
        None
    }
}

fn relationship_summary(
    source_name: &str,
    target_name: &str,
    kind: &OntologyRelationshipKind,
) -> String {
    match kind {
        OntologyRelationshipKind::OpposesCharacter => {
            format!("{source_name} opposes {target_name}.")
        }
        OntologyRelationshipKind::AdvisesCharacter => {
            format!("{source_name} advises {target_name}.")
        }
        OntologyRelationshipKind::SupportsCharacter => {
            format!("{source_name} supports {target_name}.")
        }
        OntologyRelationshipKind::SiblingOfCharacter => {
            format!("{source_name} is {target_name}'s sibling.")
        }
        _ => format!("{source_name} relates to {target_name}."),
    }
}

fn relationship_label(relationship: &OntologyRelationship) -> String {
    match relationship.kind {
        OntologyRelationshipKind::OpposesCharacter => "Character relationship".to_string(),
        OntologyRelationshipKind::AdvisesCharacter => "Advisor relationship".to_string(),
        OntologyRelationshipKind::SupportsCharacter => "Support relationship".to_string(),
        OntologyRelationshipKind::SiblingOfCharacter => "Sibling relationship".to_string(),
        _ => "Relationship".to_string(),
    }
}

fn suggested_target(prompt: &str, is_relationship_statement: bool) -> NarrativeCommitTarget {
    if is_relationship_statement {
        return NarrativeCommitTarget::Character;
    }

    let lowered = prompt.to_lowercase();
    let event_markers = [
        "scene",
        "event",
        "happens",
        "happen",
        "when",
        "after",
        "before",
        "during",
        "attack",
        "meeting",
        "arrival",
        "battle",
        "confrontation",
    ];

    if event_markers.iter().any(|marker| lowered.contains(marker)) {
        NarrativeCommitTarget::Event
    } else {
        NarrativeCommitTarget::Character
    }
}

#[derive(Clone)]
struct RelationshipStatement {
    subject: String,
    object: String,
    kind: RelationshipStatementKind,
}

#[derive(Clone, Copy)]
enum RelationshipStatementKind {
    Sibling,
}

fn extract_relationship_statement(prompt: &str) -> Option<RelationshipStatement> {
    let words = prompt.split_whitespace().collect::<Vec<_>>();
    if words.len() < 4 {
        return None;
    }

    let subject = title_like_name(words[0])?;
    let lowered = prompt.to_lowercase();

    if let Some(index) = lowered.find("'s brother") {
        let before = prompt[..index].trim();
        let owner = before
            .split_whitespace()
            .last()
            .and_then(title_like_name)?;
        return Some(RelationshipStatement {
            subject,
            object: owner,
            kind: RelationshipStatementKind::Sibling,
        });
    }

    if let Some(index) = lowered.find("'s sister") {
        let before = prompt[..index].trim();
        let owner = before
            .split_whitespace()
            .last()
            .and_then(title_like_name)?;
        return Some(RelationshipStatement {
            subject,
            object: owner,
            kind: RelationshipStatementKind::Sibling,
        });
    }

    if lowered.contains(" brother of ") || lowered.contains(" sister of ") || lowered.contains(" sibling of ") {
        let relation_index = [" brother of ", " sister of ", " sibling of "]
            .iter()
            .find_map(|marker| lowered.find(marker).map(|index| (marker, index)))?;
        let object = prompt[relation_index.1 + relation_index.0.len()..]
            .split_whitespace()
            .next()
            .and_then(title_like_name)?;
        return Some(RelationshipStatement {
            subject,
            object,
            kind: RelationshipStatementKind::Sibling,
        });
    }

    None
}

fn title_like_name(token: &str) -> Option<String> {
    let cleaned = token.trim_matches(|character: char| !character.is_alphanumeric() && character != '\'' && character != '-');
    let starts_uppercase = cleaned
        .chars()
        .next()
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false);

    if cleaned.is_empty() || !starts_uppercase {
        None
    } else {
        Some(cleaned.to_string())
    }
}
