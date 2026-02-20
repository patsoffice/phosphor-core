use std::collections::HashMap;

use phosphor_core::core::machine::InputButton;
use sdl2::keyboard::Scancode;

/// Maps SDL scancodes to machine button IDs.
pub struct KeyMap {
    map: HashMap<Scancode, u8>,
}

impl KeyMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Bind a scancode to a machine button ID.
    pub fn bind(&mut self, scancode: Scancode, button_id: u8) {
        self.map.insert(scancode, button_id);
    }

    /// Look up the machine button ID for a scancode.
    pub fn get(&self, scancode: Scancode) -> Option<u8> {
        self.map.get(&scancode).copied()
    }
}

/// Build a default key map for a machine's input buttons.
/// Uses name-based matching: common button names across machines
/// get consistent default bindings without game-specific knowledge.
pub fn default_key_map(buttons: &[InputButton]) -> KeyMap {
    let mut km = KeyMap::new();

    for button in buttons {
        let scancode = match button.name {
            // Player 1
            "P1 Left" => Some(Scancode::Left),
            "P1 Right" => Some(Scancode::Right),
            "P1 Up" => Some(Scancode::Up),
            "P1 Down" => Some(Scancode::Down),
            "P1 Flap" => Some(Scancode::Space),
            "P1 Jump" => Some(Scancode::Space),
            "P1 Fire" => Some(Scancode::LCtrl),
            "P1 Start" => Some(Scancode::Num1),

            // Player 2
            "P2 Left" => Some(Scancode::A),
            "P2 Right" => Some(Scancode::D),
            "P2 Up" => Some(Scancode::W),
            "P2 Down" => Some(Scancode::S),
            "P2 Flap" => Some(Scancode::W),
            "P2 Jump" => Some(Scancode::E),
            "P2 Fire" => Some(Scancode::E),
            "P2 Start" => Some(Scancode::Num2),

            // Fire stick (Robotron twin-stick)
            "P1 Fire Up" => Some(Scancode::I),
            "P1 Fire Down" => Some(Scancode::K),
            "P1 Fire Left" => Some(Scancode::J),
            "P1 Fire Right" => Some(Scancode::L),

            // Missile Command fire buttons
            "Fire Left" => Some(Scancode::Z),
            "Fire Center" => Some(Scancode::X),
            "Fire Right" => Some(Scancode::C),

            // System
            "Coin" => Some(Scancode::Num5),

            _ => None,
        };

        if let Some(sc) = scancode {
            km.bind(sc, button.id);
        }
    }

    km
}
