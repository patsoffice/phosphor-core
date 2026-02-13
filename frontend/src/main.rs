use phosphor_core::core::machine::Machine;
use phosphor_machines::JoustSystem;
use phosphor_machines::joust::{JOUST_BANKED_ROM, JOUST_DECODER_PROM, JOUST_SOUND_ROM};

mod emulator;
mod input;
mod rom_path;
mod video;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Usage: phosphor <machine> <rom-path> [--scale N]

    let machine_name = args
        .get(1)
        .expect("Usage: phosphor <machine> <rom-path> [--scale N]");
    let rom_path = args.get(2).expect("ROM path required");
    let scale = parse_scale_arg(&args).unwrap_or(3);

    let mut machine: Box<dyn Machine> = match machine_name.as_str() {
        "joust" => {
            let rom_set = rom_path::load_rom_set("joust", rom_path).expect("Failed to load ROMs");

            // Validate all ROM regions (even those not yet wired into memory)
            JOUST_BANKED_ROM
                .load(&rom_set)
                .expect("Failed to load banked ROMs");
            JOUST_SOUND_ROM
                .load(&rom_set)
                .expect("Failed to load sound ROM");
            JOUST_DECODER_PROM
                .load(&rom_set)
                .expect("Failed to load decoder PROMs");

            let mut sys = JoustSystem::new();
            sys.load_rom_set(&rom_set)
                .expect("Failed to map program ROMs");
            Box::new(sys)
        }
        _ => {
            eprintln!("Unknown machine: {}", machine_name);
            eprintln!("Available: joust");
            std::process::exit(1);
        }
    };

    let key_map = input::default_key_map(machine.input_map());
    machine.reset();
    emulator::run(machine.as_mut(), &key_map, scale);
}

fn parse_scale_arg(args: &[String]) -> Option<u32> {
    args.windows(2).find_map(|w| {
        if w[0] == "--scale" {
            w[1].parse().ok()
        } else {
            None
        }
    })
}
