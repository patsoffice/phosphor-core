// MAME state.h shim for standalone mame4all M6809 cross-validation.
// All state save/load functions are no-ops for validation.

#ifndef STATE_H
#define STATE_H

// --- State save stubs (new-style, from state.h) ---
#define state_save_register_UINT8(mod, inst, name, ptr, cnt)   ((void)0)
#define state_save_register_UINT16(mod, inst, name, ptr, cnt)  ((void)0)
#define state_save_register_INT32(mod, inst, name, ptr, cnt)   ((void)0)
#define state_save_register_int(mod, inst, name, ptr)          ((void)0)
#define state_save_register_func_postload(fn)                  ((void)0)

// --- State save/load stubs (old-style) ---
#define state_save_UINT8(file, mod, cpu, name, ptr, cnt)       ((void)0)
#define state_save_UINT16(file, mod, cpu, name, ptr, cnt)      ((void)0)
#define state_load_UINT8(file, mod, cpu, name, ptr, cnt)       ((void)0)
#define state_load_UINT16(file, mod, cpu, name, ptr, cnt)      ((void)0)

#endif // STATE_H
