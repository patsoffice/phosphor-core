-- MAME Lua script: trace Dig Dug IO state machine + position
-- Usage: mame digdug -autoboot_script tools/mame_digdug_trace.lua
--
-- Dumps ROM around ctrl write addresses for disassembly, then per-frame:
--   - Reads game IO state variable at 0x8900
--   - Reads 06XX ctrl register (0x7100)
--   - Reads Z80 alternate registers (BC' = transfer count)
--   - Tracks sprite 0 position

local frame_count = 0
local prev_x = 0
local prev_y = 0
local started = false

local POS_RAM_BASE = 0x9000
local SPRITE0_Y = POS_RAM_BASE + 0x380
local SPRITE0_X = POS_RAM_BASE + 0x381

local setup_done = false

function hex_dump(mem, start, len)
    local parts = {}
    for i = 0, len - 1 do
        parts[#parts + 1] = string.format("%02X", mem:read_u8(start + i))
    end
    return table.concat(parts, " ")
end

function do_setup()
    if setup_done then return end
    local cpu = manager.machine.devices[":maincpu"]
    if not cpu then return end
    local mem = cpu.spaces["program"]
    if not mem then return end

    -- Dump ROM around ctrl write addresses for disassembly
    -- NMI handler area (0x0060-0x0100)
    print("[ROM] 0x0060: " .. hex_dump(mem, 0x0060, 32))
    print("[ROM] 0x0080: " .. hex_dump(mem, 0x0080, 32))
    print("[ROM] 0x00A0: " .. hex_dump(mem, 0x00A0, 32))
    print("[ROM] 0x00C0: " .. hex_dump(mem, 0x00C0, 32))
    print("[ROM] 0x00E0: " .. hex_dump(mem, 0x00E0, 32))
    print("[ROM] 0x0100: " .. hex_dump(mem, 0x0100, 32))

    -- Main game code ctrl write areas
    print("[ROM] 0x3870: " .. hex_dump(mem, 0x3870, 32))
    print("[ROM] 0x3890: " .. hex_dump(mem, 0x3890, 32))
    print("[ROM] 0x39A0: " .. hex_dump(mem, 0x39A0, 32))
    print("[ROM] 0x39C0: " .. hex_dump(mem, 0x39C0, 32))
    print("[ROM] 0x3A50: " .. hex_dump(mem, 0x3A50, 32))
    print("[ROM] 0x3A60: " .. hex_dump(mem, 0x3A60, 32))
    print("[ROM] 0x3A80: " .. hex_dump(mem, 0x3A80, 32))

    -- Also dump the IO state table area near 0x8900
    print("[ROM] IO state ptr area 0x8900-0x890F: " .. hex_dump(mem, 0x8900, 16))

    setup_done = true
    print("[setup] ROM dump complete.")
end

function frame_callback()
    frame_count = frame_count + 1

    if not setup_done then do_setup() end

    local cpu = manager.machine.devices[":maincpu"]
    if not cpu then return end
    local mem = cpu.spaces["program"]
    if not mem then return end

    local x = mem:read_u8(SPRITE0_X)
    local y = mem:read_u8(SPRITE0_Y)

    -- Detect P1 Start or auto-start
    if not started then
        local inp = manager.machine.ioport.ports[":IN1"]
        if inp then
            local in1_val = inp:read()
            if (in1_val & 0x04) == 0 then
                started = true
                print(string.format("[DIAG] === P1 START detected at frame %d ===", frame_count))
            end
        end
        if frame_count > 800 then
            started = true
            print(string.format("[DIAG] === Auto-start at frame %d ===", frame_count))
        end
    end

    if started then
        -- Read 06XX ctrl register
        local ctrl = mem:read_u8(0x7100)

        -- Read IO state variables
        -- 0x8900: pointer used by NMI handler completion check
        local io_state_ptr_lo = mem:read_u8(0x8900)
        local io_state_ptr_hi = mem:read_u8(0x8901)
        local io_state_ptr = io_state_ptr_lo + io_state_ptr_hi * 256

        -- Read the value at the IO state pointer
        local io_state_val = 0
        if io_state_ptr >= 0x8000 and io_state_ptr < 0x9000 then
            io_state_val = mem:read_u8(io_state_ptr)
        end

        -- Read IO buffer area (likely near 0x8800-0x880F based on typical Namco layout)
        local buf = hex_dump(mem, 0x8800, 16)

        -- Read Z80 PC and SP for context
        local pc = cpu.state["PC"].value
        local sp = cpu.state["SP"].value

        -- Log every frame with all state
        print(string.format(
            "[STATE] frame=%d ctrl=0x%02X io_ptr=0x%04X io_val=0x%02X pc=0x%04X buf=[%s]",
            frame_count, ctrl, io_state_ptr, io_state_val, pc, buf))

        -- Log position changes
        if x ~= prev_x or y ~= prev_y then
            local dx = x - prev_x
            local dy = y - prev_y
            if dx > 127 then dx = dx - 256 end
            if dx < -128 then dx = dx + 256 end
            if dy > 127 then dy = dy - 256 end
            if dy < -128 then dy = dy + 256 end
            print(string.format("[POS] frame=%d x=%d y=%d dx=%d dy=%d",
                frame_count, x, y, dx, dy))
        end
    end

    prev_x = x
    prev_y = y
end

emu.register_frame_done(frame_callback, "digdug_trace")
print("[digdug_trace] Loaded. Will dump ROM + track IO state per frame.")
