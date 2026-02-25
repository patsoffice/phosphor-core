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

/// Cached register snapshot for one CPU.
pub struct CpuPanel {
    pub name: String,
    pub registers: Vec<DebugRegister>,
}

/// Cached register snapshot for one peripheral device.
pub struct DevicePanel {
    pub name: String,
    pub registers: Vec<DebugRegister>,
}

/// Persistent state for the debug UI across frames.
pub struct DebugState {
    pub active: bool,
    pub run_mode: RunMode,
    pub cpu_panels: Vec<CpuPanel>,
    pub device_panels: Vec<DevicePanel>,
    pub step_cpu: usize,
    pub cycle_count: u64,
}

impl DebugState {
    pub fn new() -> Self {
        Self {
            active: false,
            run_mode: RunMode::Running,
            cpu_panels: Vec::new(),
            device_panels: Vec::new(),
            step_cpu: 0,
            cycle_count: 0,
        }
    }

    /// Refresh cached state from the BusDebug interface.
    pub fn refresh(&mut self, bus: &dyn BusDebug) {
        // Collect CPU names for filtering devices
        let cpus = bus.cpus();
        let cpu_names: Vec<&str> = cpus.iter().map(|(name, _)| *name).collect();

        self.cpu_panels = cpus
            .iter()
            .map(|(name, cpu)| CpuPanel {
                name: name.to_string(),
                registers: cpu.debug_registers(),
            })
            .collect();

        // Device panels exclude CPUs (they're already shown in cpu_panels)
        self.device_panels = bus
            .devices()
            .iter()
            .filter(|(name, _)| !cpu_names.contains(name))
            .map(|(name, dev)| DevicePanel {
                name: name.to_string(),
                registers: dev.debug_registers(),
            })
            .collect();

        // Clamp step_cpu to valid range
        if self.step_cpu >= self.cpu_panels.len() && !self.cpu_panels.is_empty() {
            self.step_cpu = 0;
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
            let cpf = machine.cycles_per_frame();
            if cpf > 0 {
                for _ in 0..cpf {
                    machine.debug_tick();
                    state.cycle_count += 1;
                }
            } else {
                machine.run_frame();
            }
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
                if (boundaries >> state.step_cpu) & 1 != 0 {
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

/// Draw a register grid for a set of debug registers.
fn draw_register_grid(ui: &mut egui::Ui, id: &str, registers: &[DebugRegister]) {
    egui::Grid::new(id)
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for reg in registers {
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
            egui::ScrollArea::vertical().show(ui, |ui| {
                // CPU panels (collapsible, default open)
                for (i, panel) in state.cpu_panels.iter().enumerate() {
                    let id = egui::Id::new(format!("cpu_{i}"));
                    egui::CollapsingHeader::new(
                        egui::RichText::new(&panel.name).monospace().strong(),
                    )
                    .id_salt(id)
                    .default_open(true)
                    .show(ui, |ui| {
                        draw_register_grid(ui, &format!("cpu_regs_{i}"), &panel.registers);
                    });
                }

                // Step-CPU target (only for multi-CPU machines)
                if state.cpu_panels.len() > 1 {
                    ui.separator();
                    ui.label("Step target:");
                    ui.horizontal(|ui| {
                        for (i, panel) in state.cpu_panels.iter().enumerate() {
                            ui.radio_value(&mut state.step_cpu, i, &panel.name);
                        }
                    });
                }

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

                // Device panels (collapsible, default closed)
                if !state.device_panels.is_empty() {
                    ui.separator();
                    for (i, panel) in state.device_panels.iter().enumerate() {
                        let id = egui::Id::new(format!("dev_{i}"));
                        egui::CollapsingHeader::new(egui::RichText::new(&panel.name).monospace())
                            .id_salt(id)
                            .default_open(false)
                            .show(ui, |ui| {
                                draw_register_grid(ui, &format!("dev_regs_{i}"), &panel.registers);
                            });
                    }
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
