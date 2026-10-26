#[cfg(test)]
mod tests {
    use crate::{
        config::{capability_map::LayerActivationMode, DeviceProfile},
        input::{
            capability::{Capability, Gamepad, GamepadButton, Keyboard},
            composite_device::layers::{LayerState, MAX_ACTIVE_LAYERS},
            event::{native::NativeEvent, value::InputValue},
        },
    };

    fn profile(yaml: &str) -> DeviceProfile {
        DeviceProfile::from_yaml(yaml.to_string()).expect("parse profile")
    }

    fn button(name: &str) -> Capability {
        let button = match name {
            "Guide" => GamepadButton::Guide,
            "QuickAccess" => GamepadButton::QuickAccess,
            "South" => GamepadButton::South,
            _ => panic!("unknown button {name}"),
        };
        Capability::Gamepad(Gamepad::Button(button))
    }

    fn key(name: &str) -> Capability {
        let key = match name {
            "Esc" => Keyboard::KeyEsc,
            "Space" => Keyboard::KeySpace,
            "Shift" => Keyboard::KeyLeftShift,
            _ => panic!("unknown key {name}"),
        };
        Capability::Keyboard(key)
    }

    /// A pressed event a layer produced for a source capability.
    fn output(source: &Capability, target: &str) -> NativeEvent {
        NativeEvent::new_translated(source.clone(), key(target), InputValue::Bool(true))
    }

    const TEST_PROFILE: &str = r#"
version: 1
kind: DeviceProfile
name: Layer state test
mapping:
  - name: Guide opens guide
    source_event: { gamepad: { button: Guide } }
    target_events:
      - layer: { name: guide }
  - name: QuickAccess latches mouse_mode
    source_event: { gamepad: { button: QuickAccess } }
    target_events:
      - layer: { name: mouse_mode, mode: toggle }
layers:
  - name: guide
    mapping:
      - name: South -> Esc
        source_event: { gamepad: { button: South } }
        target_events:
          - keyboard: KeyEsc
  - name: rapid
    mapping:
      - name: South -> Space
        source_event: { gamepad: { button: South } }
        target_events:
          - keyboard: KeySpace
  - name: mouse_mode
    mapping:
      - name: South -> Shift
        source_event: { gamepad: { button: South } }
        target_events:
          - keyboard: KeyLeftShift
"#;

    fn state() -> LayerState {
        let mut state = LayerState::default();
        state.load_profile(Some(&profile(TEST_PROFILE)));
        state
    }

    /// Extra hold layers to fill the stack, all remapping South.
    fn extra_layers(yaml: &str, count: usize) -> String {
        let mut yaml = String::from(yaml);
        for index in 1..=count {
            yaml.push_str(&format!(
                "  - name: extra{index}\n    mapping:\n      - name: m{index}\n        source_event: \
                 {{ gamepad: {{ button: South }} }}\n        target_events:\n          - keyboard: KeyEsc\n"
            ));
        }
        yaml
    }

    #[test]
    fn hold_activation_pushes_and_pops() {
        let mut state = state();
        let guide = button("Guide");

        state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);
        assert_eq!(state.active_names(), ["guide"]);

        // Releasing the activator pops the layer again.
        state.release_activation(&guide);
        assert!(state.active_names().is_empty());

        // Releasing an untracked input changes nothing.
        let releases = state.release_activation(&guide);
        assert!(releases.is_empty());
        assert!(state.active_names().is_empty());
    }

    #[test]
    fn toggle_activation_latches_across_release() {
        let mut state = state();
        let quick_access = button("QuickAccess");
        let mode = LayerActivationMode::Toggle;

        state.apply_activation(&quick_access, "mouse_mode", &mode, true);
        assert_eq!(state.active_names(), ["mouse_mode"]);

        // Auto-repeat while the button is still held must not toggle it back off.
        state.apply_activation(&quick_access, "mouse_mode", &mode, true);
        assert_eq!(state.active_names(), ["mouse_mode"]);

        // The layer survives the release of the activating input.
        let releases = state.release_activation(&quick_access);
        assert!(releases.is_empty());
        assert_eq!(state.active_names(), ["mouse_mode"]);

        // The next press releases it.
        state.apply_activation(&quick_access, "mouse_mode", &mode, true);
        assert!(state.active_names().is_empty());
    }

    #[test]
    fn top_most_active_layer_wins_per_source() {
        let mut state = state();
        let guide = button("Guide");
        let rapid = button("South");

        state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);
        assert_eq!(state.mapping_for(&rapid).unwrap().0, "guide");

        // Activating `rapid` later shadows `guide` for the shared source.
        state.activate("rapid");
        assert_eq!(state.mapping_for(&rapid).unwrap().0, "rapid");

        // Removing the top layer uncovers the layer underneath.
        state.deactivate("rapid");
        assert_eq!(state.mapping_for(&rapid).unwrap().0, "guide");

        // Sources no layer maps have no winner.
        assert!(state.mapping_for(&button("QuickAccess")).is_none());
    }

    #[test]
    fn teardown_releases_held_outputs_in_reverse_order() {
        let mut state = state();
        let guide = button("Guide");
        let south = button("South");

        state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);
        state.record_outputs(
            &south,
            Some("guide"),
            true,
            &[output(&south, "Esc"), output(&south, "Shift")],
        );

        let releases = state.release_activation(&guide);
        assert_eq!(releases.len(), 2);
        for event in &releases {
            assert!(!event.pressed());
        }
        assert_eq!(releases[0].as_capability(), key("Shift"));
        assert_eq!(releases[1].as_capability(), key("Esc"));

        // The recorded outputs are gone, so a second teardown releases nothing.
        assert!(state.clear().is_empty());
    }

    #[test]
    fn teardown_only_releases_outputs_of_the_popped_layer() {
        let mut state = state();
        let south = button("South");

        state.activate("guide");
        state.activate("rapid");
        state.record_outputs(&south, Some("rapid"), true, &[output(&south, "Space")]);

        // Popping the lower layer must not release what the upper layer holds.
        let releases = state.deactivate("guide");
        assert!(releases.is_empty());
        assert_eq!(state.mapping_for(&south).unwrap().0, "rapid");
    }

    #[test]
    fn depth_cap_evicts_bottom_most_hold_layer() {
        let mut state = state();
        state.load_profile(Some(&profile(&extra_layers(TEST_PROFILE, 8))));

        for index in 1..=8 {
            state.activate(&format!("extra{index}"));
        }
        assert_eq!(state.active_names().len(), MAX_ACTIVE_LAYERS);
        assert_eq!(state.active_names()[0], "extra1");

        // The layer being evicted holds an output, which must be released.
        let south = button("South");
        state.record_outputs(&south, Some("extra1"), true, &[output(&south, "Esc")]);

        state.activate("guide");
        assert_eq!(state.active_names().len(), MAX_ACTIVE_LAYERS);
        assert_eq!(state.active_names()[0], "extra2");
        assert_eq!(state.active_names().last().unwrap(), "guide");
    }

    /// Evicting a layer must release what it produced while held, otherwise the
    /// target device is left with an input stuck down.
    #[test]
    fn evicting_a_layer_releases_its_held_outputs() {
        let mut state = state();
        state.load_profile(Some(&profile(&extra_layers(TEST_PROFILE, 8))));

        for index in 1..=8 {
            state.activate(&format!("extra{index}"));
        }

        let south = button("South");
        state.record_outputs(&south, Some("extra1"), true, &[output(&south, "Esc")]);

        let releases = state.activate("guide");
        assert_eq!(releases.len(), 1);
        assert!(!releases[0].pressed());
        assert_eq!(releases[0].as_capability(), key("Esc"));
    }

    /// A single input can activate more than one layer, and releasing it must
    /// undo all of the hold activations it made.
    #[test]
    fn releasing_a_source_undoes_every_activation_it_holds() {
        let mut state = state();
        let guide = button("Guide");
        let mode = LayerActivationMode::Hold;

        state.apply_activation(&guide, "guide", &mode, true);
        state.apply_activation(&guide, "rapid", &mode, true);
        assert_eq!(state.active_names(), ["guide", "rapid"]);

        state.release_activation(&guide);
        assert!(state.active_names().is_empty());
    }

    /// The release of a held activation reaches the state machine twice: the
    /// guard in `process_event`, then the same event through the activation
    /// path. The layer must stay popped.
    #[test]
    fn release_does_not_reactivate_a_popped_hold_layer() {
        let mut state = state();
        let guide = button("Guide");
        let mode = LayerActivationMode::Hold;

        state.apply_activation(&guide, "mouse_mode", &mode, true);
        assert_eq!(state.active_names(), ["mouse_mode"]);

        state.release_activation(&guide);

        let releases = state.apply_activation(&guide, "mouse_mode", &mode, false);
        assert!(releases.is_empty());
        assert!(state.active_names().is_empty());
    }

    /// A hold layer still pops when only the activation path sees the release,
    /// e.g. when an active layer shadows the activation mapping.
    #[test]
    fn release_through_activation_path_pops_hold_layer() {
        let mut state = state();
        let guide = button("Guide");
        let mode = LayerActivationMode::Hold;

        state.apply_activation(&guide, "mouse_mode", &mode, true);
        state.apply_activation(&guide, "mouse_mode", &mode, false);
        assert!(state.active_names().is_empty());
    }

    /// A release must never flip a latched toggle layer back off.
    #[test]
    fn release_does_not_toggle_a_latched_layer_off() {
        let mut state = state();
        let quick_access = button("QuickAccess");
        let mode = LayerActivationMode::Toggle;

        state.apply_activation(&quick_access, "mouse_mode", &mode, true);
        assert_eq!(state.active_names(), ["mouse_mode"]);

        state.apply_activation(&quick_access, "mouse_mode", &mode, false);
        assert_eq!(state.active_names(), ["mouse_mode"]);

        state.apply_activation(&quick_access, "mouse_mode", &mode, true);
        assert!(state.active_names().is_empty());
    }

    #[test]
    fn unknown_layers_are_rejected() {
        let mut state = state();

        // An activation of an unknown layer from a mapping is a no-op.
        let guide = button("Guide");
        let releases = state.apply_activation(&guide, "nope", &LayerActivationMode::Hold, true);
        assert!(releases.is_empty());
        assert!(state.active_names().is_empty());
    }

    #[test]
    fn clear_pops_every_layer_and_releases_outputs() {
        let mut state = state();
        let south = button("South");

        state.activate("guide");
        state.activate("rapid");
        state.record_outputs(&south, Some("rapid"), true, &[output(&south, "Space")]);

        let releases = state.clear();
        assert_eq!(releases.len(), 1);
        assert!(!releases[0].pressed());
        assert!(state.active_names().is_empty());
    }

    /// The events an activation mapping produces for a source: holding `layer`
    /// down, plus the source's own button as a gamepad output.
    fn activation_with_tap(
        source: &Capability,
        layer: &str,
        mode: LayerActivationMode,
        pressed: bool,
    ) -> Vec<NativeEvent> {
        vec![
            NativeEvent::new_translated(
                source.clone(),
                Capability::Layer {
                    name: layer.to_string(),
                    mode,
                },
                InputValue::Bool(pressed),
            ),
            NativeEvent::new_translated(source.clone(), source.clone(), InputValue::Bool(pressed)),
        ]
    }

    fn is_layer(event: &NativeEvent) -> bool {
        matches!(event.as_capability(), Capability::Layer { .. })
    }

    #[test]
    fn tapped_activator_emits_its_held_back_outputs_on_release() {
        let mut state = state();
        let guide = button("Guide");

        // The press only holds the layer down, the Guide output waits.
        let now = state.defer_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, true),
        );
        assert_eq!(now.len(), 1);
        assert!(is_layer(&now[0]));
        state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);

        // Released with no layer output in between: Guide goes out as a tap.
        state.release_activation(&guide);
        let (now, tap) = state.finish_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, false),
        );
        assert_eq!(now.len(), 1);
        assert!(is_layer(&now[0]));
        assert_eq!(tap.len(), 2);
        assert_eq!(tap[0].as_capability(), guide);
        assert!(tap[0].pressed());
        assert_eq!(tap[1].as_capability(), guide);
        assert!(!tap[1].pressed());
    }

    #[test]
    fn used_layer_drops_the_held_back_outputs() {
        let mut state = state();
        let guide = button("Guide");
        let south = button("South");

        state.defer_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, true),
        );
        state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);
        state.record_outputs(&south, Some("guide"), true, &[output(&south, "Esc")]);

        state.release_activation(&guide);
        let (now, tap) = state.finish_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, false),
        );
        assert!(now.iter().all(is_layer));
        assert!(tap.is_empty());

        // The next tap starts clean.
        state.defer_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, true),
        );
        state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);
        state.release_activation(&guide);
        let (_, tap) = state.finish_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, false),
        );
        assert_eq!(tap.len(), 2);
    }

    #[test]
    fn base_outputs_while_held_do_not_count_as_a_chord() {
        let mut state = state();
        let guide = button("Guide");
        let quick_access = button("QuickAccess");

        state.defer_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, true),
        );
        state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);
        // An input the layer does not map translates through the base profile.
        state.record_outputs(&quick_access, None, true, &[output(&quick_access, "Esc")]);

        state.release_activation(&guide);
        let (_, tap) = state.finish_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, false),
        );
        assert_eq!(tap.len(), 2);
    }

    #[test]
    fn repeated_press_keeps_one_deferral() {
        let mut state = state();
        let guide = button("Guide");

        for _ in 0..3 {
            state.defer_tap(
                &guide,
                activation_with_tap(&guide, "guide", LayerActivationMode::Hold, true),
            );
            state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);
        }
        state.release_activation(&guide);
        let (_, tap) = state.finish_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, false),
        );
        assert_eq!(tap.len(), 2);
    }

    #[test]
    fn only_hold_activations_of_known_layers_defer() {
        let mut state = state();
        let guide = button("Guide");
        let quick_access = button("QuickAccess");

        // A toggle activation passes its outputs on at once.
        let now = state.defer_tap(
            &quick_access,
            activation_with_tap(
                &quick_access,
                "mouse_mode",
                LayerActivationMode::Toggle,
                true,
            ),
        );
        assert_eq!(now.len(), 2);

        // So does an activation of a layer the profile does not define.
        let now = state.defer_tap(
            &guide,
            activation_with_tap(&guide, "nope", LayerActivationMode::Hold, true),
        );
        assert_eq!(now.len(), 2);

        // And a release with nothing held back is untouched.
        let (now, tap) = state.finish_tap(
            &guide,
            activation_with_tap(&guide, "nope", LayerActivationMode::Hold, false),
        );
        assert_eq!(now.len(), 2);
        assert!(tap.is_empty());
    }

    #[test]
    fn clear_drops_a_pending_tap() {
        let mut state = state();
        let guide = button("Guide");

        state.defer_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, true),
        );
        state.apply_activation(&guide, "guide", &LayerActivationMode::Hold, true);
        state.clear();

        let (now, tap) = state.finish_tap(
            &guide,
            activation_with_tap(&guide, "guide", LayerActivationMode::Hold, false),
        );
        assert_eq!(now.len(), 2);
        assert!(tap.is_empty());
    }
}
