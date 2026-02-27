use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, parse_macro_input};

/// Derive macro that generates a `BusDebug` implementation for a struct.
///
/// Annotate fields with:
/// - `#[debug_device("Name")]` — field implements `Device`, listed in `devices()`.
///   Also generates `write_device_register()` and `reset_device()` dispatch.
/// - `#[debug_cpu("Name")]` — field implements `DebugCpu`, listed in both `devices()`
///   AND `cpus()`. Debug reads/writes are auto-routed through the matching
///   `#[debug_map(cpu = N)]` field's `MemoryMap::debug_read`/`debug_write`.
/// - `#[debug_cpu("Name", read = "method", write = "method")]` — explicit version:
///   names `&self` / `&mut self` methods on the struct for side-effect-free memory access.
/// - `#[debug_map(cpu = N)]` — field is a `MemoryMap` linked to CPU index N.
///   Generates watchpoint routing and (when linked to a `#[debug_cpu]`) debug memory access.
///
/// CPU index assignment is positional: first `#[debug_cpu]` is index 0, etc.
/// Device indices for `write_device_register` / `reset_device` match `devices()` order.
#[proc_macro_derive(BusDebug, attributes(debug_device, debug_cpu, debug_map))]
pub fn derive_bus_debug(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("BusDebug can only be derived on structs with named fields"),
        },
        _ => panic!("BusDebug can only be derived on structs"),
    };

    let mut device_entries = Vec::new(); // (name, field_ident, is_device) for all annotated fields
    let mut cpu_entries: Vec<(
        syn::LitStr,
        syn::Ident,
        Option<syn::LitStr>,
        Option<syn::LitStr>,
    )> = Vec::new(); // (name, field_ident, read_method?, write_method?)
    let mut map_entries: Vec<MapEntry> = Vec::new(); // (cpu_index, field_ident) for MemoryMap fields

    for field in fields {
        let field_ident = field.ident.as_ref().expect("named field");

        for attr in &field.attrs {
            if attr.path().is_ident("debug_device") {
                // #[debug_device("Name")] — field implements Device
                let name: syn::LitStr = attr
                    .parse_args()
                    .expect("debug_device expects a string literal: #[debug_device(\"Name\")]");
                device_entries.push((name, field_ident.clone(), true));
            } else if attr.path().is_ident("debug_cpu") {
                // #[debug_cpu("Name")] or #[debug_cpu("Name", read = "method", write = "method")]
                let args: CpuArgs = attr
                    .parse_args()
                    .expect("debug_cpu expects: (\"Name\") or (\"Name\", read = \"method\", write = \"method\")");
                // CPUs appear in both devices() and cpus()
                device_entries.push((args.name.clone(), field_ident.clone(), false));
                cpu_entries.push((args.name, field_ident.clone(), args.read, args.write));
            } else if attr.path().is_ident("debug_map") {
                // #[debug_map(cpu = N)] — field is a MemoryMap linked to CPU index N
                let args: MapArgs = attr.parse_args().expect("debug_map expects: (cpu = N)");
                map_entries.push(MapEntry {
                    cpu_index: args.cpu_index,
                    field_ident: field_ident.clone(),
                });
            }
        }
    }

    // Generate devices() body
    let device_items = device_entries.iter().map(|(name, ident, _)| {
        quote! { (#name, &self.#ident as &dyn phosphor_core::core::debug::Debuggable) }
    });

    // Generate cpus() body
    let cpu_items = cpu_entries.iter().map(|(name, ident, _, _)| {
        quote! { (#name, &self.#ident as &dyn phosphor_core::core::debug::DebugCpu) }
    });

    // Generate read() match arms
    let read_arms: Vec<_> = cpu_entries
        .iter()
        .enumerate()
        .map(|(i, (_, _, read_method, _))| {
            let idx = i;
            if let Some(read_method) = read_method {
                // Explicit method: self.method(addr)
                let read_ident =
                    syn::Ident::new(read_method.value().as_str(), read_method.span());
                quote! { #idx => self.#read_ident(addr) }
            } else {
                // Auto-route through matching #[debug_map(cpu = N)]
                let map_field = map_entries
                    .iter()
                    .find(|m| m.cpu_index == i)
                    .unwrap_or_else(|| {
                        panic!(
                            "debug_cpu at index {i} has no read method and no matching #[debug_map(cpu = {i})]"
                        )
                    });
                let map_ident = &map_field.field_ident;
                quote! { #idx => self.#map_ident.debug_read(addr) }
            }
        })
        .collect();

    // Generate write() match arms
    let write_arms: Vec<_> = cpu_entries
        .iter()
        .enumerate()
        .map(|(i, (_, _, _, write_method))| {
            let idx = i;
            if let Some(write_method) = write_method {
                // Explicit method: self.method(addr, data)
                let write_ident =
                    syn::Ident::new(write_method.value().as_str(), write_method.span());
                quote! { #idx => self.#write_ident(addr, data) }
            } else {
                // Auto-route through matching #[debug_map(cpu = N)]
                let map_field = map_entries
                    .iter()
                    .find(|m| m.cpu_index == i)
                    .unwrap_or_else(|| {
                        panic!(
                            "debug_cpu at index {i} has no write method and no matching #[debug_map(cpu = {i})]"
                        )
                    });
                let map_ident = &map_field.field_ident;
                quote! { #idx => self.#map_ident.debug_write(addr, data) }
            }
        })
        .collect();

    // Generate write_device_register() match arms (only #[debug_device] fields)
    let device_write_arms =
        device_entries
            .iter()
            .enumerate()
            .filter_map(|(i, (_, ident, is_device))| {
                if *is_device {
                    let idx = i;
                    Some(quote! {
                        #idx => phosphor_core::device::Device::write(&mut self.#ident, offset, data)
                    })
                } else {
                    None
                }
            });

    // Generate reset_device() match arms (only #[debug_device] fields)
    let device_reset_arms =
        device_entries
            .iter()
            .enumerate()
            .filter_map(|(i, (_, ident, is_device))| {
                if *is_device {
                    let idx = i;
                    Some(quote! {
                        #idx => phosphor_core::device::Device::reset(&mut self.#ident)
                    })
                } else {
                    None
                }
            });

    // Generate watchpoint methods (only when #[debug_map] fields exist)
    let watchpoint_methods = if !map_entries.is_empty() {
        // take_watchpoint_hit: chain .or_else() across all maps (declaration order)
        let take_hit_chain = map_entries.iter().map(|entry| {
            let ident = &entry.field_ident;
            quote! { .or_else(|| self.#ident.take_hit()) }
        });

        // set_watchpoint / clear_watchpoint: match on cpu_index
        let set_arms = map_entries.iter().map(|entry| {
            let idx = entry.cpu_index;
            let ident = &entry.field_ident;
            quote! { #idx => self.#ident.set_watchpoint(addr, kind) }
        });
        let clear_arms = map_entries.iter().map(|entry| {
            let idx = entry.cpu_index;
            let ident = &entry.field_ident;
            quote! { #idx => self.#ident.clear_watchpoint(addr, kind) }
        });

        // clear_all_watchpoints: call on every map
        let clear_all_calls = map_entries.iter().map(|entry| {
            let ident = &entry.field_ident;
            quote! { self.#ident.clear_all_watchpoints(); }
        });

        // memory_map: match on cpu_index
        let map_arms = map_entries.iter().map(|entry| {
            let idx = entry.cpu_index;
            let ident = &entry.field_ident;
            quote! { #idx => Some(&self.#ident) }
        });

        quote! {
            fn take_watchpoint_hit(&mut self) -> Option<phosphor_core::core::memory_map::WatchpointHit> {
                None #(#take_hit_chain)*
            }

            fn set_watchpoint(&mut self, cpu_index: usize, addr: u16, kind: phosphor_core::core::memory_map::WatchpointKind) {
                match cpu_index {
                    #(#set_arms,)*
                    _ => {}
                }
            }

            fn clear_watchpoint(&mut self, cpu_index: usize, addr: u16, kind: phosphor_core::core::memory_map::WatchpointKind) {
                match cpu_index {
                    #(#clear_arms,)*
                    _ => {}
                }
            }

            fn clear_all_watchpoints(&mut self) {
                #(#clear_all_calls)*
            }

            fn memory_map(&self, cpu_index: usize) -> Option<&phosphor_core::core::memory_map::MemoryMap> {
                match cpu_index {
                    #(#map_arms,)*
                    _ => None,
                }
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        impl phosphor_core::core::debug::BusDebug for #struct_name {
            fn devices(&self) -> Vec<(&str, &dyn phosphor_core::core::debug::Debuggable)> {
                vec![#(#device_items),*]
            }

            fn cpus(&self) -> Vec<(&str, &dyn phosphor_core::core::debug::DebugCpu)> {
                vec![#(#cpu_items),*]
            }

            fn read(&self, cpu_index: usize, addr: u16) -> Option<u8> {
                match cpu_index {
                    #(#read_arms,)*
                    _ => None,
                }
            }

            fn write(&mut self, cpu_index: usize, addr: u16, data: u8) {
                match cpu_index {
                    #(#write_arms,)*
                    _ => {}
                }
            }

            fn write_device_register(&mut self, device_index: usize, offset: u16, data: u8) {
                match device_index {
                    #(#device_write_arms,)*
                    _ => {}
                }
            }

            fn reset_device(&mut self, device_index: usize) {
                match device_index {
                    #(#device_reset_arms,)*
                    _ => {}
                }
            }

            #watchpoint_methods
        }
    };

    TokenStream::from(expanded)
}

/// Parsed arguments for `#[debug_cpu("Name")]` or `#[debug_cpu("Name", read = "method", write = "method")]`.
struct CpuArgs {
    name: syn::LitStr,
    read: Option<syn::LitStr>,
    write: Option<syn::LitStr>,
}

impl syn::parse::Parse for CpuArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: syn::LitStr = input.parse()?;

        // If no comma follows, this is the short form: #[debug_cpu("Name")]
        if input.is_empty() || !input.peek(syn::Token![,]) {
            return Ok(CpuArgs {
                name,
                read: None,
                write: None,
            });
        }

        input.parse::<syn::Token![,]>()?;

        let mut read = None;
        let mut write = None;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;
            let value: syn::LitStr = input.parse()?;

            match key.to_string().as_str() {
                "read" => read = Some(value),
                "write" => write = Some(value),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown attribute `{other}`, expected `read` or `write`"),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
            }
        }

        // If read or write is provided, both must be present
        if read.is_some() != write.is_some() {
            return Err(input.error("both `read` and `write` must be specified, or neither"));
        }

        Ok(CpuArgs { name, read, write })
    }
}

/// Collected info for a `#[debug_map(cpu = N)]` field.
struct MapEntry {
    cpu_index: usize,
    field_ident: syn::Ident,
}

/// Parsed arguments for `#[debug_map(cpu = N)]`.
struct MapArgs {
    cpu_index: usize,
}

impl syn::parse::Parse for MapArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let key: syn::Ident = input.parse()?;
        if key != "cpu" {
            return Err(syn::Error::new(
                key.span(),
                format!("unknown attribute `{key}`, expected `cpu`"),
            ));
        }
        input.parse::<syn::Token![=]>()?;
        let value: syn::LitInt = input.parse()?;
        Ok(MapArgs {
            cpu_index: value.base10_parse()?,
        })
    }
}

/// Derive macro that generates boilerplate for memory region ID enums.
///
/// Given a `#[repr(u8)]` enum, generates:
/// - `impl From<EnumName> for u8` (casting via `as u8`)
/// - Associated `u8` constants in SCREAMING_SNAKE_CASE for each variant
///   (e.g., `Region::VideoRam` → `Region::VIDEO_RAM`)
///
/// The constants inherit the enum's visibility.
#[proc_macro_derive(MemoryRegion)]
pub fn derive_memory_region(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = &input.ident;
    let vis = &input.vis;

    let variants = match &input.data {
        syn::Data::Enum(data) => &data.variants,
        _ => panic!("MemoryRegion can only be derived on enums"),
    };

    // Generate associated constants (PascalCase → SCREAMING_SNAKE_CASE)
    let const_items = variants.iter().map(|v| {
        let variant_name = &v.ident;
        let const_name = syn::Ident::new(
            &pascal_to_screaming_snake(&variant_name.to_string()),
            variant_name.span(),
        );
        quote! {
            #[allow(dead_code)]
            #vis const #const_name: u8 = Self::#variant_name as u8;
        }
    });

    let expanded = quote! {
        impl #enum_name {
            #(#const_items)*
        }

        impl From<#enum_name> for u8 {
            fn from(r: #enum_name) -> u8 {
                r as u8
            }
        }
    };

    TokenStream::from(expanded)
}

/// Convert PascalCase to SCREAMING_SNAKE_CASE.
///
/// Inserts `_` before each uppercase letter that follows a lowercase letter.
fn pascal_to_screaming_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 && s.as_bytes()[i - 1].is_ascii_lowercase() {
            result.push('_');
        }
        result.extend(c.to_uppercase());
    }
    result
}
