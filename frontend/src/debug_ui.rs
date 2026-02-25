use phosphor_core::core::debug::{BusDebug, DebugRegister};
use phosphor_core::core::machine::Machine;

/// Execution modes for the debug interface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunMode {
    Running,
    Paused,
    StepInstruction,
    StepCycle,
}

/// Persistent state for the debug UI across frames.
pub struct DebugState {
    pub active: bool,
    pub run_mode: RunMode,
    pub registers: Vec<DebugRegister>,
    pub selected_cpu: usize,
    pub cpu_count: usize,
    pub cpu_name: String,
    pub cycle_count: u64,
}

impl DebugState {
    pub fn new() -> Self {
        Self {
            active: false,
            run_mode: RunMode::Running,
            registers: Vec::new(),
            selected_cpu: 0,
            cpu_count: 1,
            cpu_name: String::from("CPU"),
            cycle_count: 0,
        }
    }

    /// Refresh cached state from the BusDebug interface.
    pub fn refresh(&mut self, bus: &dyn BusDebug) {
        let cpus = bus.cpus();
        self.cpu_count = cpus.len();
        if let Some((name, cpu)) = cpus.get(self.selected_cpu) {
            self.cpu_name = name.to_string();
            self.registers = cpu.debug_registers();
        } else if let Some((name, cpu)) = cpus.first() {
            self.selected_cpu = 0;
            self.cpu_name = name.to_string();
            self.registers = cpu.debug_registers();
        }
    }
}

/// Execute one frame of emulation according to the current run mode.
/// Returns true if a full frame was executed (caller should drain audio).
pub fn execute_frame(machine: &mut dyn Machine, state: &mut DebugState) -> bool {
    if !state.active {
        machine.run_frame();
        return true;
    }

    match state.run_mode {
        RunMode::Running => {
            machine.run_frame();
            if let Some(bus) = machine.debug_bus() {
                state.refresh(bus);
            }
            true
        }
        RunMode::Paused => {
            if let Some(bus) = machine.debug_bus() {
                state.refresh(bus);
            }
            false
        }
        RunMode::StepInstruction => {
            loop {
                let boundaries = machine.debug_tick();
                state.cycle_count += 1;
                if (boundaries >> state.selected_cpu) & 1 != 0 {
                    break;
                }
            }
            if let Some(bus) = machine.debug_bus() {
                state.refresh(bus);
            }
            state.run_mode = RunMode::Paused;
            false
        }
        RunMode::StepCycle => {
            machine.debug_tick();
            state.cycle_count += 1;
            if let Some(bus) = machine.debug_bus() {
                state.refresh(bus);
            }
            state.run_mode = RunMode::Paused;
            false
        }
    }
}

/// Build the debug UI layout. Called as the closure argument to Video::present_with_debug().
pub fn draw_debug_ui(
    ctx: &egui::Context,
    game_texture_id: egui::TextureId,
    native_size: (u32, u32),
    state: &mut DebugState,
) {
    // Right panel: registers and controls
    egui::SidePanel::right("debug_panel")
        .default_width(220.0)
        .resizable(true)
        .show(ctx, |ui| {
            // CPU selector (multi-CPU machines)
            if state.cpu_count > 1 {
                ui.horizontal(|ui| {
                    for i in 0..state.cpu_count {
                        let label = if i == state.selected_cpu {
                            egui::RichText::new(&state.cpu_name).strong()
                        } else {
                            egui::RichText::new(format!("CPU {i}"))
                        };
                        if ui
                            .selectable_label(state.selected_cpu == i, label)
                            .clicked()
                        {
                            state.selected_cpu = i;
                        }
                    }
                });
                ui.separator();
            } else {
                ui.label(egui::RichText::new(&state.cpu_name).monospace().strong());
                ui.separator();
            }

            // Registers
            egui::Grid::new("register_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for reg in &state.registers {
                        ui.label(egui::RichText::new(reg.name).monospace());
                        let value_text = match reg.width {
                            8 => format!("${:02X}", reg.value),
                            16 => format!("${:04X}", reg.value),
                            _ => format!("${:X}", reg.value),
                        };
                        ui.label(egui::RichText::new(value_text).monospace());
                        ui.end_row();
                    }
                });

            ui.separator();
            ui.label(format!("Cycles: {}", state.cycle_count));
            ui.separator();

            // Step controls
            let is_paused = state.run_mode == RunMode::Paused;

            ui.horizontal(|ui| {
                if state.run_mode == RunMode::Running {
                    if ui.button("Pause").clicked() {
                        state.run_mode = RunMode::Paused;
                    }
                } else if ui.button("Continue (F4)").clicked() {
                    state.run_mode = RunMode::Running;
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(is_paused, egui::Button::new("Step Instr (F2)"))
                    .clicked()
                {
                    state.run_mode = RunMode::StepInstruction;
                }
                if ui
                    .add_enabled(is_paused, egui::Button::new("Step Cycle (F3)"))
                    .clicked()
                {
                    state.run_mode = RunMode::StepCycle;
                }
            });
        });

    // Central panel: game framebuffer with aspect ratio preservation
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
        .show(ctx, |ui| {
            let available = ui.available_size();
            let (nw, nh) = native_size;
            let aspect = nw as f32 / nh as f32;
            let (display_w, display_h) = if available.x / available.y > aspect {
                (available.y * aspect, available.y)
            } else {
                (available.x, available.x / aspect)
            };

            // Center the image
            let offset_x = (available.x - display_w) / 2.0;
            let offset_y = (available.y - display_h) / 2.0;
            ui.add_space(offset_y);
            ui.horizontal(|ui| {
                ui.add_space(offset_x);
                ui.image(egui::load::SizedTexture::new(
                    game_texture_id,
                    egui::Vec2::new(display_w, display_h),
                ));
            });
        });
}
