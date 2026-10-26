use std::collections::HashMap;

use crate::{
    config::{capability_map::LayerActivationMode, DeviceProfile, ProfileMapping},
    input::{
        capability::Capability,
        event::{native::NativeEvent, value::InputValue},
    },
};

use super::index_mappings_by_source;

/// Guard against pathological configs and runaway DBus clients.
pub(crate) const MAX_ACTIVE_LAYERS: usize = 8;

/// A layer currently on the stack.
#[derive(Debug, Clone)]
struct ActiveLayer {
    name: String,
    /// How the layer was activated. Decides release handling and depth-cap
    /// eviction.
    mode: LayerActivationMode,
}

/// The other outputs of a mapping that also holds a layer down, kept back until
/// the activator is released. They go out as a tap only if none of the layers it
/// activated produced anything in between, so one button can both open a layer
/// (held with others) and keep its own meaning (tapped alone).
#[derive(Debug, Clone)]
struct DeferredTap {
    /// The held-back press events, in emission order.
    presses: Vec<NativeEvent>,
    /// Set once a layer this activator holds produced an output.
    chorded: bool,
}

/// Owns all input layer state for a [super::CompositeDevice]: the layer lookup
/// tables from the loaded profile, the stack of active layers, and the inputs
/// those layers produced. Every layer decision lives here; the composite device
/// only applies the results. Layer state changes return the release events they
/// oblige the caller to emit, in emission order (last output first).
#[derive(Debug, Default)]
pub struct LayerState {
    /// Layer name -> map of source capability -> that layer's mappings.
    config_maps: HashMap<String, HashMap<Capability, Vec<ProfileMapping>>>,
    /// The active layers as a stack: bottom (activated first) to top (activated
    /// last). Precedence for a source capability is top-down.
    active_layers: Vec<ActiveLayer>,
    /// Source capability -> the activations it currently holds down. A source
    /// can drive more than one activation.
    pressed_activators: HashMap<Capability, Vec<(String, LayerActivationMode)>>,
    /// (source capability, layer) -> target capabilities that layer emitted for
    /// that source, so they can be released if the layer leaves the stack while
    /// the source is still held. Base-profile translations are deliberately not
    /// tracked here.
    active_inputs: HashMap<(Capability, String), Vec<Capability>>,
    /// Activator source capability -> the outputs of its mapping held back
    /// until it is released (see [DeferredTap]).
    deferred_taps: HashMap<Capability, DeferredTap>,
}

impl LayerState {
    /// Rebuild all layer state from the given profile, resetting the stack.
    /// Invalid config is warned about and skipped, never fatal.
    pub fn load_profile(&mut self, profile: Option<&DeviceProfile>) {
        self.config_maps.clear();
        self.active_layers.clear();
        self.pressed_activators.clear();
        self.active_inputs.clear();
        self.deferred_taps.clear();

        let Some(profile) = profile else {
            return;
        };

        // Index each layer's mappings by source capability.
        let mut names: Vec<&str> = Vec::new();
        for layer in profile.layers.iter() {
            if layer.name.is_empty() {
                log::warn!("Skipping layer in profile with an empty name");
                continue;
            }
            if names.contains(&layer.name.as_str()) {
                log::warn!("Skipping duplicate layer `{}` in profile", layer.name);
                continue;
            }
            names.push(layer.name.as_str());
            self.config_maps
                .insert(layer.name.clone(), index_mappings_by_source(&layer.mapping));
        }

        // Check every activation the profile's base mappings declare.
        let mut modes: HashMap<&str, LayerActivationMode> = HashMap::new();
        for mapping in profile.mapping.iter() {
            let source_cap: Capability = mapping.source_event.clone().into();
            for target in mapping.target_events.iter() {
                let Some(layer) = target.layer.as_ref() else {
                    continue;
                };
                if !self.config_maps.contains_key(&layer.name) {
                    log::warn!(
                        "Mapping `{}` activates unknown layer `{}`, ignoring activation",
                        mapping.name,
                        layer.name
                    );
                    continue;
                }

                // A layer activated with two different modes is ambiguous.
                let mode = layer.mode.clone().unwrap_or_default();
                match modes.get(layer.name.as_str()) {
                    Some(existing) if *existing != mode => log::warn!(
                        "Layer `{}` is activated with conflicting modes by mapping `{}`",
                        layer.name,
                        mapping.name
                    ),
                    None => {
                        modes.insert(layer.name.as_str(), mode);
                    }
                    _ => {}
                }

                // While a layer is active its mapping wins, which hides the
                // activation mapping for that source.
                if self
                    .config_maps
                    .values()
                    .any(|config_map| config_map.contains_key(&source_cap))
                {
                    log::warn!(
                        "Layer `{}` remaps {}, which activates a layer. That activation is hidden while the layer is active.",
                        layer.name,
                        source_cap.to_capability_string()
                    );
                }
            }
        }
    }

    /// The active stack, bottom (first activated) to top (last activated).
    #[cfg(test)]
    pub fn active_names(&self) -> Vec<String> {
        self.active_layers
            .iter()
            .map(|layer| layer.name.clone())
            .collect()
    }

    /// Push a layer onto the stack by name.
    #[cfg(test)]
    pub fn activate(&mut self, name: &str) -> Vec<NativeEvent> {
        self.push_layer(name, &LayerActivationMode::Hold)
    }

    /// Remove a layer from wherever it sits in the stack.
    #[cfg(test)]
    pub fn deactivate(&mut self, name: &str) -> Vec<NativeEvent> {
        self.pop_layer(name)
    }

    /// Apply an activation produced by a mapping. Only presses reach here;
    /// releases are handled by [LayerState::release_activation].
    pub fn apply_activation(
        &mut self,
        source: &Capability,
        name: &str,
        mode: &LayerActivationMode,
        pressed: bool,
    ) -> Vec<NativeEvent> {
        // A release never activates a layer, it ends a held activation. Handling
        // it here keeps the outcome correct whichever entry point sees the
        // release event first, and idempotent when both do.
        if !pressed {
            return self.release_activation(source);
        }

        if !self.config_maps.contains_key(name) {
            log::warn!("Ignoring activation of unknown layer `{name}`");
            return Vec::new();
        }

        // An activator that is already held down means this press repeats.
        let repeated = self
            .pressed_activators
            .get(source)
            .is_some_and(|activations| !activations.is_empty());
        self.remember_activator(source, name, mode);

        match mode {
            LayerActivationMode::Hold => self.push_layer(name, mode),
            // Repeats of a held activator must not toggle the layer back off.
            LayerActivationMode::Toggle if repeated => Vec::new(),
            LayerActivationMode::Toggle => {
                if self.active_layers.iter().any(|layer| layer.name == name) {
                    self.pop_layer(name)
                } else {
                    self.push_layer(name, mode)
                }
            }
        }
    }

    /// Forget a held activator and, for a hold activation, pop the layer. This
    /// is the single place a held activation is released, so it also covers the
    /// case where an active layer shadows the activation mapping.
    pub fn release_activation(&mut self, source: &Capability) -> Vec<NativeEvent> {
        let Some(activations) = self.pressed_activators.remove(source) else {
            return Vec::new();
        };

        let mut releases = Vec::new();
        // Undo the most recent activation first, matching emission order.
        for (name, mode) in activations.into_iter().rev() {
            if mode == LayerActivationMode::Hold {
                releases.extend(self.pop_layer(&name));
            }
        }
        releases
    }

    /// Returns the winning layer for a source capability: the top-most active
    /// layer that maps it, and that layer's mappings for it.
    pub fn mapping_for(&self, source_cap: &Capability) -> Option<(&str, &[ProfileMapping])> {
        for layer in self.active_layers.iter().rev() {
            let Some(config_map) = self.config_maps.get(&layer.name) else {
                continue;
            };
            if let Some(mappings) = config_map.get(source_cap) {
                return Some((layer.name.as_str(), mappings));
            }
        }
        None
    }

    /// Track what the winning layer emitted for a source event, so removing that
    /// layer can release it while the source is still held. A release means the
    /// output already went out through the normal event path.
    pub fn record_outputs(
        &mut self,
        source: &Capability,
        layer: Option<&str>,
        pressed: bool,
        events: &[NativeEvent],
    ) {
        if !pressed {
            self.active_inputs
                .retain(|(input_source, _), _| input_source != source);
            return;
        }

        let Some(layer) = layer else {
            return;
        };

        // Layer activations are consumed by the composite device, they can never
        // need a release.
        let produced: Vec<Capability> = events
            .iter()
            .map(|event| event.as_capability())
            .filter(|cap| !matches!(cap, Capability::Layer { .. }))
            .collect();
        if produced.is_empty() {
            return;
        }

        self.active_inputs
            .insert((source.clone(), layer.to_string()), produced);

        // The layer was used, so whatever holds it down is a chord, not a tap.
        for (activator, activations) in self.pressed_activators.iter() {
            if !activations.iter().any(|(name, _)| name == layer) {
                continue;
            }
            if let Some(tap) = self.deferred_taps.get_mut(activator) {
                tap.chorded = true;
            }
        }
    }

    /// Hold back the outputs a hold activation's mapping produces besides the
    /// activation itself, until [LayerState::finish_tap]. Returns the events to
    /// process now: the activation alone, or `events` untouched when they hold
    /// no layer down or carry nothing else.
    pub fn defer_tap(&mut self, source: &Capability, events: Vec<NativeEvent>) -> Vec<NativeEvent> {
        let holds_layer = events.iter().any(|event| {
            matches!(event.as_capability(),
                Capability::Layer { name, mode: LayerActivationMode::Hold }
                    if self.config_maps.contains_key(&name))
        });
        if !holds_layer {
            return events;
        }
        let (activations, others): (Vec<NativeEvent>, Vec<NativeEvent>) = events
            .into_iter()
            .partition(|event| matches!(event.as_capability(), Capability::Layer { .. }));
        if !others.is_empty() {
            // A repeated press of a held activator keeps the first deferral.
            self.deferred_taps
                .entry(source.clone())
                .or_insert(DeferredTap {
                    presses: others,
                    chorded: false,
                });
        }
        activations
    }

    /// The release of an input whose outputs [LayerState::defer_tap] held back.
    /// Returns the events to process now (the release minus the held-back
    /// outputs, which never went out) and the tap to emit: the held-back
    /// presses then their releases in reverse order, or nothing when a layer
    /// was used while the input was held.
    pub fn finish_tap(
        &mut self,
        source: &Capability,
        events: Vec<NativeEvent>,
    ) -> (Vec<NativeEvent>, Vec<NativeEvent>) {
        let Some(tap) = self.deferred_taps.remove(source) else {
            return (events, Vec::new());
        };
        let activations: Vec<NativeEvent> = events
            .into_iter()
            .filter(|event| matches!(event.as_capability(), Capability::Layer { .. }))
            .collect();
        if tap.chorded {
            return (activations, Vec::new());
        }

        let releases: Vec<NativeEvent> = tap
            .presses
            .iter()
            .rev()
            .map(|press| {
                NativeEvent::new_translated(
                    source.clone(),
                    press.as_capability(),
                    InputValue::Bool(false),
                )
            })
            .collect();
        let mut sequence = tap.presses;
        sequence.extend(releases);
        (activations, sequence)
    }

    /// Remove a layer from anywhere in the stack and return the outputs it must
    /// release. Outputs produced by other layers or the base profile are left
    /// alone.
    fn pop_layer(&mut self, name: &str) -> Vec<NativeEvent> {
        let Some(index) = self
            .active_layers
            .iter()
            .position(|layer| layer.name == name)
        else {
            return Vec::new();
        };
        self.active_layers.remove(index);
        self.release_outputs(name)
    }

    /// Pop every active layer from the top of the stack down, returning one
    /// combined set of releases so the caller emits a single signal.
    pub fn clear(&mut self) -> Vec<NativeEvent> {
        // A tap still held back never went out, so it needs no release.
        self.deferred_taps.clear();
        let mut releases = Vec::new();
        while let Some(layer) = self.active_layers.last() {
            let name = layer.name.clone();
            releases.extend(self.pop_layer(&name));
        }
        releases
    }

    /// Append a layer to the top of the stack. A layer already in the stack is
    /// moved to the top and its recorded mode is replaced.
    fn push_layer(&mut self, name: &str, mode: &LayerActivationMode) -> Vec<NativeEvent> {
        if let Some(index) = self
            .active_layers
            .iter()
            .position(|layer| layer.name == name)
        {
            self.active_layers.remove(index);
            self.active_layers.push(ActiveLayer {
                name: name.to_string(),
                mode: mode.clone(),
            });
            return Vec::new();
        }

        let mut releases = Vec::new();

        // Enforce the depth cap: evict the bottom-most hold layer, since toggle
        // layers were activated deliberately.
        if self.active_layers.len() >= MAX_ACTIVE_LAYERS {
            let Some(evicted) = self
                .active_layers
                .iter()
                .find(|layer| layer.mode == LayerActivationMode::Hold)
                .map(|layer| layer.name.clone())
            else {
                log::warn!(
                    "Refusing to activate layer `{name}`: all {MAX_ACTIVE_LAYERS} active layers are toggle layers"
                );
                return releases;
            };
            log::warn!(
                "Layer stack is full ({MAX_ACTIVE_LAYERS}), evicting bottom-most hold layer `{evicted}` to activate `{name}`"
            );
            releases.extend(self.pop_layer(&evicted));
        }

        self.active_layers.push(ActiveLayer {
            name: name.to_string(),
            mode: mode.clone(),
        });
        releases
    }

    /// Remember that the given input is holding an activation down.
    fn remember_activator(&mut self, source: &Capability, name: &str, mode: &LayerActivationMode) {
        let activations = self.pressed_activators.entry(source.clone()).or_default();
        if !activations
            .iter()
            .any(|(layer, m)| layer == name && m == mode)
        {
            activations.push((name.to_string(), mode.clone()));
        }
    }

    /// Build the release events for everything `name` produced while held.
    fn release_outputs(&mut self, name: &str) -> Vec<NativeEvent> {
        let held: Vec<(Capability, Vec<Capability>)> = self
            .active_inputs
            .iter()
            .filter(|((_, layer), _)| layer == name)
            .map(|((source, _), produced)| (source.clone(), produced.clone()))
            .collect();

        let mut releases = Vec::new();
        for (source_cap, produced) in held {
            self.active_inputs
                .remove(&(source_cap.clone(), name.to_string()));
            // Release in reverse emission order.
            for target_cap in produced.iter().rev() {
                releases.push(NativeEvent::new_translated(
                    source_cap.clone(),
                    target_cap.clone(),
                    InputValue::Bool(false),
                ));
            }
        }
        releases
    }
}
