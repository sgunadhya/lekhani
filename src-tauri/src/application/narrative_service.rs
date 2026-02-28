use crate::domain::{
    NarrativeChangeKind, NarrativeChangeSummary, NarrativeCharacter, NarrativeCommitTarget,
    NarrativeEvent, NarrativeMessagePreview, NarrativeNudge, NarrativeSnapshot,
    OntologyRelationship, OntologyRelationshipKind,
};
use crate::ports::{CharacterParser, EventParser, NudgeGenerator};

pub struct NarrativeService<C, E, N> {
    character_parser: C,
    event_parser: E,
    nudge_generator: N,
}

impl<C, E, N> NarrativeService<C, E, N> {
    pub fn new(character_parser: C, event_parser: E, nudge_generator: N) -> Self {
        Self {
            character_parser,
            event_parser,
            nudge_generator,
        }
    }
}

impl<C, E, N> NarrativeService<C, E, N>
where
    C: CharacterParser,
    E: EventParser,
    N: NudgeGenerator,
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

    pub fn get_nudge(&self, snapshot: &NarrativeSnapshot) -> Result<NarrativeNudge, String> {
        self.nudge_generator.generate_nudge(snapshot)
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
            });
        }

        let character = self
            .parse_character(&prompt, snapshot)
            .ok()
            .map(|character| merge_character(character, snapshot));
        let event = self
            .parse_event(&prompt, snapshot)
            .ok()
            .map(|event| merge_event(event, snapshot));
        let suggested_target = suggested_target(&prompt);
        let relationships = infer_relationships(&prompt, snapshot);
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
) -> Vec<OntologyRelationship> {
    let lowered = prompt.to_lowercase();
    let Some(kind) = infer_relationship_kind(&lowered) else {
        return Vec::new();
    };

    let matched = snapshot
        .characters
        .iter()
        .filter(|character| lowered.contains(&character.name.to_lowercase()))
        .collect::<Vec<_>>();

    if matched.len() < 2 {
        return Vec::new();
    }

    let source = matched[0];
    let target = matched[1];
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

fn infer_relationship_kind(prompt: &str) -> Option<OntologyRelationshipKind> {
    if ["opposes", "against", "enemy", "rival", "conflicts with"]
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
        _ => format!("{source_name} relates to {target_name}."),
    }
}

fn relationship_label(relationship: &OntologyRelationship) -> String {
    match relationship.kind {
        OntologyRelationshipKind::OpposesCharacter => "Character relationship".to_string(),
        OntologyRelationshipKind::AdvisesCharacter => "Advisor relationship".to_string(),
        OntologyRelationshipKind::SupportsCharacter => "Support relationship".to_string(),
        _ => "Relationship".to_string(),
    }
}

fn suggested_target(prompt: &str) -> NarrativeCommitTarget {
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
