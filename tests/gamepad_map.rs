//! Gamepad mapping tests — hardware-free, because the mapping layer is pure.
//! The `gilrs` adapter in `render` only fills in a `PadSnapshot`; everything
//! that decides *what a button does* is covered here.

use overframe::config::Settings;
use overframe::gamepad::{
    apply_deadzone, map, mask_label, merge, PadAction, PadBindings, PadButton, PadSnapshot,
    TRIGGER_THRESHOLD,
};
use overframe::sim::{buttons, PlayerInput, Vec2};

fn snap_with(buttons: u32) -> PadSnapshot {
    PadSnapshot {
        buttons,
        ..Default::default()
    }
}

#[test]
fn default_layout_mirrors_the_classic_controller() {
    let b = PadBindings::default();
    assert_eq!(
        map(&snap_with(PadButton::South.mask()), &b).buttons,
        buttons::ATTACK
    );
    assert_eq!(
        map(&snap_with(PadButton::East.mask()), &b).buttons,
        buttons::SPECIAL
    );
    assert_eq!(
        map(&snap_with(PadButton::North.mask()), &b).buttons,
        buttons::JUMP
    );
    assert_eq!(
        map(&snap_with(PadButton::West.mask()), &b).buttons,
        buttons::JUMP
    );
    assert_eq!(
        map(&snap_with(PadButton::RightBumper.mask()), &b).buttons,
        buttons::GRAB
    );
    assert_eq!(
        map(&snap_with(PadButton::LeftBumper.mask()), &b).buttons,
        buttons::SHIELD
    );
    // Unbound buttons do nothing.
    assert_eq!(map(&snap_with(PadButton::Start.mask()), &b).buttons, 0);
}

#[test]
fn analog_triggers_shield_past_threshold() {
    let b = PadBindings::default();
    let mut s = PadSnapshot::default();
    s.left_trigger = TRIGGER_THRESHOLD - 0.1;
    assert_eq!(map(&s, &b).buttons, 0, "light pull is not a press");
    s.left_trigger = TRIGGER_THRESHOLD + 0.1;
    assert_eq!(map(&s, &b).buttons, buttons::SHIELD);
    s.left_trigger = 0.0;
    s.right_trigger = 1.0;
    assert_eq!(map(&s, &b).buttons, buttons::SHIELD);
}

#[test]
fn sticks_map_with_radial_deadzone_and_rescale() {
    let b = PadBindings::default();
    let mut s = PadSnapshot::default();
    s.left = Vec2::new(0.1, 0.1); // inside deadzone
    assert_eq!(map(&s, &b).stick, Vec2::ZERO);

    s.left = Vec2::new(1.0, 0.0);
    let out = map(&s, &b).stick;
    assert!(
        (out.x - 1.0).abs() < 1e-5 && out.y.abs() < 1e-5,
        "full deflection stays full"
    );

    s.left = Vec2::new(0.0, 0.6);
    let out = map(&s, &b).stick;
    assert!(
        out.y > 0.0 && out.y < 0.6,
        "rescaled from the deadzone edge, got {}",
        out.y
    );

    s.right = Vec2::new(0.0, -1.0);
    assert!(
        (map(&s, &b).cstick.y + 1.0).abs() < 1e-5,
        "right stick is the C-stick"
    );

    // Deadzone helper directly.
    assert_eq!(apply_deadzone(Vec2::new(0.19, 0.0), 0.2), Vec2::ZERO);
    let e = apply_deadzone(Vec2::new(0.2, 0.0), 0.2);
    assert!(e.length() < 1e-6, "edge of deadzone maps to zero");
}

#[test]
fn rebinding_moves_a_button_between_actions() {
    let mut b = PadBindings::default();
    // Put the jump on A (South). It must stop attacking.
    b.rebind(PadAction::Jump, PadButton::South);
    assert_eq!(
        map(&snap_with(PadButton::South.mask()), &b).buttons,
        buttons::JUMP
    );
    assert_eq!(b.get(PadAction::Attack) & PadButton::South.mask(), 0);
    assert_eq!(
        b.get(PadAction::Jump),
        PadButton::South.mask(),
        "exactly one button now"
    );
    // Labels are human readable.
    assert_eq!(mask_label(b.get(PadAction::Jump)), "A / SOUTH");
    assert_eq!(mask_label(0), "(NONE)");
    assert!(mask_label(PadBindings::default().jump).contains('+'));
}

#[test]
fn bindings_persist_through_settings_file() {
    let mut s = Settings::default();
    s.pad[1].rebind(PadAction::Grab, PadButton::LeftThumb);
    s.pad[1].deadzone = 0.33;
    let back = Settings::from_text(&s.to_text());
    assert_eq!(
        back.pad[1].get(PadAction::Grab),
        PadButton::LeftThumb.mask()
    );
    assert!((back.pad[1].deadzone - 0.33).abs() < 1e-6);
    assert_eq!(back.pad[0], PadBindings::default());
}

#[test]
fn keyboard_and_gamepad_merge_so_either_works() {
    let kb = PlayerInput {
        stick: Vec2::new(0.3, 0.0),
        buttons: buttons::JUMP,
        ..Default::default()
    };
    let pad = PlayerInput {
        stick: Vec2::new(0.0, -0.9),
        buttons: buttons::ATTACK,
        ..Default::default()
    };
    let m = merge(kb, pad);
    assert_eq!(m.buttons, buttons::JUMP | buttons::ATTACK);
    assert_eq!(m.stick, pad.stick, "stronger stick wins");
    assert_eq!(
        merge(kb, PlayerInput::default()),
        kb,
        "idle pad changes nothing"
    );
}

#[test]
fn every_button_has_a_stable_index_and_label() {
    for (i, b) in PadButton::ALL.iter().enumerate() {
        assert_eq!(PadButton::from_index(i as u32), Some(*b));
        assert_eq!(b.mask(), 1 << i);
        assert!(!b.label().is_empty());
    }
    assert_eq!(PadButton::from_index(99), None);
    for a in PadAction::ALL {
        assert!(!a.label().is_empty());
    }
}
