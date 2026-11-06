use std::collections::{HashMap, HashSet};

use evdev::InputEvent;

use crate::{
    config::capability_map::{
        evdev::{AxisDirection, ValueType},
        CapabilityMapConfig, CapabilityMapConfigV1, CapabilityMapConfigV2, EvdevMappingType,
    },
    input::{
        capability::Capability,
        event::{
            evdev::{EvdevEvent, EventSignature},
            EventCapability,
        },
    },
};

/// V1 one-to-one mappings are single source chords; there is no OneToOne
/// variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MappingKind {
    Chord,
    MultiSource,
}

/// One normalized mapping from either map version. Multi source mappings
/// are split into one entry per source event so each carries its own value
/// conversion.
#[derive(Debug, Clone)]
pub struct MappedTranslation {
    name: String,
    pub(super) kind: MappingKind,
    /// The press is deferred while a strict superset chord may complete.
    pub(super) deferrable: bool,
    /// One entry per source event; any listed signature satisfies it.
    pub(super) source_events: Vec<(Vec<EventSignature>, Capability)>,
    pub(super) target: Capability,
    /// Value conversion, set only on multi source entries.
    pub(super) value_type: Option<ValueType>,
    pub(super) axis_direction: Option<AxisDirection>,
}

/// Normalize both configs into one list.
pub fn from_maps(
    composite_device_map: Option<&CapabilityMapConfig>,
    source_device_map: Option<&CapabilityMapConfig>,
    supported: &HashSet<EventSignature>,
) -> Vec<MappedTranslation> {
    let maps: Vec<&CapabilityMapConfig> = [composite_device_map, source_device_map]
        .into_iter()
        .flatten()
        .collect();

    let mut duplicates: HashSet<String> = HashSet::new();
    let mut names: HashSet<String> = HashSet::new();
    for map in &maps {
        let mapping_names: Vec<&str> = match map {
            CapabilityMapConfig::V1(config) => config
                .mapping
                .iter()
                .map(|mapping| mapping.name.as_str())
                .collect(),
            CapabilityMapConfig::V2(config) => config
                .mapping
                .iter()
                .map(|mapping| mapping.name.as_str())
                .collect(),
        };
        for name in mapping_names {
            if !names.insert(name.to_owned()) {
                duplicates.insert(name.to_owned());
            }
        }
    }
    for name in &duplicates {
        log::warn!("Duplicate capability mapping name: {name}; dropping both");
    }

    let mut mappings = Vec::new();
    for map in &maps {
        mappings.extend(
            extract_map(map)
                .into_iter()
                .filter(|mapping| !duplicates.contains(&mapping.name)),
        );
    }
    let mut mappings = build_mappings(mappings);
    mappings = exclude_mismatches(mappings, supported);
    mark_deferrable(&mut mappings);
    mappings
}

/// Extract normalized mappings from a capability map.
fn extract_map(map: &CapabilityMapConfig) -> Vec<MappedTranslation> {
    match map {
        CapabilityMapConfig::V1(config) => extract_map_v1(config),
        CapabilityMapConfig::V2(config) => extract_map_v2(config),
    }
}

/// V1 sources are capabilities expanded to their evdev signatures; every V1
/// mapping is a chord.
fn extract_map_v1(config: &CapabilityMapConfigV1) -> Vec<MappedTranslation> {
    config
        .mapping
        .iter()
        .map(|mapping| {
            let source_events = mapping
                .source_events
                .iter()
                .map(|event| {
                    let capability: Capability = event.clone().into();
                    let signatures = EvdevEvent::event_codes(&capability)
                        .into_iter()
                        .map(|(event_type, code)| (event_type.0, code))
                        .collect::<Vec<_>>();
                    (signatures, capability)
                })
                .collect();
            MappedTranslation {
                name: mapping.name.clone(),
                kind: MappingKind::Chord,
                deferrable: false,
                source_events,
                target: mapping.target_event.clone().into(),
                value_type: None,
                axis_direction: None,
            }
        })
        .collect()
}

/// V2 sources name evdev events directly; sources without an evdev event
/// have no signatures and are dropped by validation. Multi source mappings
/// become one entry per source event.
fn extract_map_v2(config: &CapabilityMapConfigV2) -> Vec<MappedTranslation> {
    let mut mappings = Vec::new();
    for mapping in config.mapping.iter() {
        let kind = match mapping.mapping_type.as_ref().and_then(|t| t.evdev.as_ref()) {
            Some(EvdevMappingType::Chord) => MappingKind::Chord,
            Some(EvdevMappingType::MultiSource) | None => MappingKind::MultiSource,
        };
        let source_events = mapping
            .source_events
            .iter()
            .map(|event| match event.evdev.as_ref() {
                Some(evdev) => {
                    let signature = (evdev.event_type.as_u16(), evdev.event_code.as_u16());
                    let capability = EvdevEvent::from(InputEvent::new(signature.0, signature.1, 0))
                        .to_capability();
                    (
                        vec![signature],
                        capability,
                        Some(evdev.value_type.clone()),
                        evdev.axis_direction.clone(),
                    )
                }
                None => (Vec::new(), Capability::NotImplemented, None, None),
            })
            .collect::<Vec<_>>();
        if kind == MappingKind::MultiSource {
            for (signatures, capability, value_type, axis_direction) in source_events {
                mappings.push(MappedTranslation {
                    name: mapping.name.clone(),
                    kind,
                    deferrable: false,
                    source_events: vec![(signatures, capability)],
                    target: mapping.target_event.clone().into(),
                    value_type,
                    axis_direction,
                });
            }
            continue;
        }
        mappings.push(MappedTranslation {
            name: mapping.name.clone(),
            kind,
            deferrable: false,
            source_events: source_events
                .into_iter()
                .map(|(signatures, capability, _, _)| (signatures, capability))
                .collect(),
            target: mapping.target_event.clone().into(),
            value_type: None,
            axis_direction: None,
        });
    }
    mappings
}

/// Validate the given `Vec<MappedTranslation>` by dropping broken
/// configurations.
fn build_mappings(mut mappings: Vec<MappedTranslation>) -> Vec<MappedTranslation> {
    for mapping in mappings.iter_mut() {
        mapping.source_events.retain(|(signatures, capability)| {
            if signatures.is_empty() {
                log::warn!(
                    "Mapping {} source event {:?} has no evdev equivalent. Skipping source event.",
                    mapping.name,
                    capability
                );
                return false;
            }
            if signatures.len() > 1 {
                log::debug!(
                    "Mapping {} source event {:?} expands to multiple evdev signatures; any one satisfies it",
                    mapping.name,
                    capability
                );
            }
            true
        });
    }
    mappings.retain(|mapping| {
        let valid =
            !mapping.source_events.is_empty() && mapping.target != Capability::NotImplemented;
        if valid {
            return true;
        }
        if mapping.source_events.is_empty() {
            log::warn!(
                "Mapping {} has no translatable source events. Skipping mapping.",
                mapping.name
            );
        } else {
            log::warn!(
                "Mapping {} has no target capability. Skipping mapping.",
                mapping.name
            );
        }
        false
    });

    // Both versions claim by evdev signature. A source signature may only be
    // claimed by one mapping kind. A conflict rejects both mappings.
    let mut claimed: HashMap<EventSignature, Vec<(MappingKind, usize)>> = HashMap::new();
    let mut rejected = vec![false; mappings.len()];
    for (index, mapping) in mappings.iter().enumerate() {
        let mut conflicts: Vec<(usize, EventSignature)> = Vec::new();
        for (signatures, _) in mapping.source_events.iter() {
            for signature in signatures {
                for &(kind, other) in claimed.get(signature).into_iter().flatten() {
                    if !rejected[other] && kind != mapping.kind {
                        conflicts.push((other, *signature));
                    }
                }
            }
        }
        if !conflicts.is_empty() {
            for (other, signature) in &conflicts {
                if rejected[*other] {
                    continue;
                }
                log::error!(
                    "Mapping {} conflicts with mapping {} on signature {signature:?}. Skipping both.",
                    mappings[*other].name,
                    mapping.name
                );
                rejected[*other] = true;
            }
            rejected[index] = true;
            continue;
        }
        for (signatures, _) in mapping.source_events.iter() {
            for signature in signatures {
                claimed
                    .entry(*signature)
                    .or_default()
                    .push((mapping.kind, index));
            }
        }
    }

    let mut validated = Vec::with_capacity(mappings.len());
    for (index, mapping) in mappings.drain(..).enumerate() {
        if !rejected[index] {
            validated.push(mapping);
        }
    }
    validated
}

/// Exclude mappings that have source events the device does not support.
fn exclude_mismatches(
    mut mappings: Vec<MappedTranslation>,
    supported: &HashSet<EventSignature>,
) -> Vec<MappedTranslation> {
    mappings.retain(|mapping| {
        let available = mapping.source_events.iter().all(|(signatures, _)| {
            signatures
                .iter()
                .any(|signature| supported.contains(signature))
        });
        if !available {
            log::warn!(
                "Mapping {} has source events this device does not support; dropping",
                mapping.name
            );
        }
        available
    });
    mappings
}

/// Mark chords strictly contained by another chord in the same list.
fn mark_deferrable(mappings: &mut [MappedTranslation]) {
    for index in 0..mappings.len() {
        if mappings[index].kind != MappingKind::Chord {
            continue;
        }
        let deferrable = (0..mappings.len()).any(|other| {
            other != index
                && mappings[other].kind == MappingKind::Chord
                && translation_is_subset(&mappings[index], &mappings[other])
        });
        mappings[index].deferrable = deferrable;
    }
}

/// True when every source of `small` is also a source of `large`, and
/// `large` has at least one more.
fn translation_is_subset(small: &MappedTranslation, large: &MappedTranslation) -> bool {
    if small.source_events.len() >= large.source_events.len() {
        return false;
    }
    let mut used = vec![false; large.source_events.len()];
    for (signatures, _) in small.source_events.iter() {
        let Some(index) =
            large
                .source_events
                .iter()
                .enumerate()
                .position(|(index, (large_signatures, _))| {
                    !used[index]
                        && large_signatures.len() == signatures.len()
                        && signatures.iter().all(|s| large_signatures.contains(s))
                })
        else {
            return false;
        };
        used[index] = true;
    }
    true
}
