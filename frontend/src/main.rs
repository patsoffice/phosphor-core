use phosphor_core::core::machine::Machine;
use phosphor_machines::JoustSystem;
use phosphor_machines::rom_loader::RomSet;

mod emulator;
mod input;
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
            let rom_set = RomSet::from_directory(std::path::Path::new(rom_path))
                .expect("Failed to load ROM directory");
            let mut sys = JoustSystem::new();
            sys.load_rom_set(&rom_set).expect("Failed to map ROMs");
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
