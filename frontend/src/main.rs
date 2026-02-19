use phosphor_core::core::machine::Machine;
use phosphor_machines::DkongSystem;
use phosphor_machines::JoustSystem;
use phosphor_machines::MissileCommandSystem;
use phosphor_machines::PacmanSystem;
use phosphor_machines::joust::JOUST_DECODER_PROM;

mod audio;
mod emulator;
mod input;
mod overlay;
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

            // Validate ROM regions not yet wired into memory
            JOUST_DECODER_PROM
                .load(&rom_set)
                .expect("Failed to load decoder PROMs");

            let mut sys = JoustSystem::new();
            sys.load_rom_set(&rom_set)
                .expect("Failed to map program ROMs");
            Box::new(sys)
        }
        "missile" => {
            let rom_set = rom_path::load_rom_set("missile", rom_path).expect("Failed to load ROMs");

            let mut sys = MissileCommandSystem::new();
            sys.load_rom_set(&rom_set)
                .expect("Failed to map program ROMs");
            Box::new(sys)
        }
        "pacman" => {
            let rom_set = rom_path::load_rom_set("pacman", rom_path).expect("Failed to load ROMs");

            let mut sys = PacmanSystem::new();
            sys.load_rom_set(&rom_set).expect("Failed to map ROMs");
            Box::new(sys)
        }
        "dkong" => {
            let rom_set = rom_path::load_rom_set("dkong", rom_path).expect("Failed to load ROMs");

            let mut sys = DkongSystem::new();
            sys.load_rom_set(&rom_set).expect("Failed to map ROMs");
            Box::new(sys)
        }
        _ => {
            eprintln!("Unknown machine: {}", machine_name);
            eprintln!("Available: dkong, joust, missile, pacman");
            std::process::exit(1);
        }
    };

    // Load battery-backed NVRAM from disk (if available)
    let nvram_path = nvram_path_for(machine_name, rom_path);
    if let Ok(data) = std::fs::read(&nvram_path) {
        machine.load_nvram(&data);
    }

    let key_map = input::default_key_map(machine.input_map());
    machine.reset();
    emulator::run(machine.as_mut(), &key_map, scale);

    // Save battery-backed NVRAM to disk on exit
    if let Some(data) = machine.save_nvram()
        && let Err(e) = std::fs::write(&nvram_path, data)
    {
        eprintln!("Warning: failed to save NVRAM: {e}");
    }
}

fn nvram_path_for(machine_name: &str, rom_path: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(rom_path);
    if path.is_dir() {
        path.join(format!("{machine_name}.nvram"))
    } else {
        path.with_extension("nvram")
    }
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
