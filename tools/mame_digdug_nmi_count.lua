-- MAME Lua script: count NMIs per frame in Dig Dug
-- Usage: mame digdug -autoboot_script tools/mame_digdug_nmi_count.lua
--
-- Counts how many times PC hits 0x0066 (NMI vector) per frame.
-- This tells us whether the I/O cycle fits in 1 frame or spans 2.

local frame_count = 0
local nmi_count_this_frame = 0
local started = false

-- Track sprite 0 position for comparison
local POS_RAM_BASE = 0x9000
local SPRITE0_Y = POS_RAM_BASE + 0x380
local SPRITE0_X = POS_RAM_BASE + 0x381
local prev_x = 0
local prev_y = 0

-- NMI detection via PC breakpoint
local bp_installed = false

function install_bp()
    if bp_installed then return end
    local cpu = manager.machine.devices[":maincpu"]
    if not cpu then return end
    local dbg = cpu.debug
    if not dbg then return end

    -- Set a breakpoint at the NMI vector (0x0066)
    -- Use a passive watchpoint approach: check PC each frame instead
    bp_installed = true
    print("[nmi_count] Monitoring NMI count per frame")
end

-- Use a register-polling approach: sample PC at high frequency
-- Actually, let's use the simpler approach: read the 06XX control register
-- and count NMI deliveries by monitoring a RAM counter.

-- Better approach: monitor the NMI handler's shadow register state.
-- The NMI handler at 0x0066 uses EXX+LDI+EXX+RETN pattern.
-- We can count NMIs by monitoring writes to 0x7100 (06XX control).

local ctrl_write_count = 0
local ctrl_values = {}

function frame_callback()
    frame_count = frame_count + 1

    local cpu = manager.machine.devices[":maincpu"]
    if not cpu then return end
    local mem = cpu.spaces["program"]
    if not mem then return end

    local x = mem:read_u8(SPRITE0_X)
    local y = mem:read_u8(SPRITE0_Y)

    if started then
        -- Print NMI/IO stats every frame during gameplay
        if frame_count > 700 and frame_count < 2000 then
            local pos_str = ""
            if x ~= prev_x or y ~= prev_y then
                local dx = x - prev_x
                local dy = y - prev_y
                if dx > 127 then dx = dx - 256 end
                if dx < -128 then dx = dx + 256 end
                if dy > 127 then dy = dy - 256 end
                if dy < -128 then dy = dy + 256 end
                pos_str = string.format(" [POS x=%d y=%d dx=%d dy=%d]", x, y, dx, dy)
            end

            -- Read the 06XX control register to see current state
            local ctrl = mem:read_u8(0x7100)
            print(string.format("[FRAME] frame=%d ctrl=0x%02X%s",
                frame_count, ctrl, pos_str))
        end
    end

    if frame_count > 60 then
        started = true
    end

    prev_x = x
    prev_y = y
end

emu.register_frame_done(frame_callback, "nmi_count")
print("[nmi_count] Loaded. Will log 06XX ctrl state per frame.")
