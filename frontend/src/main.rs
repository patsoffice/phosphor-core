use phosphor_machines::registry;

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

    let entry = registry::find(machine_name).unwrap_or_else(|| {
        let names: Vec<_> = registry::all().iter().map(|e| e.name).collect();
        eprintln!("Unknown machine: {machine_name}");
        eprintln!("Available: {}", names.join(", "));
        std::process::exit(1);
    });

    let rom_set = rom_path::load_rom_set(entry.rom_name, rom_path).expect("Failed to load ROMs");
    let mut machine = (entry.create)(&rom_set).expect("Failed to initialize machine");

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
