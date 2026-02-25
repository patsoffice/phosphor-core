use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, parse_macro_input};

/// Derive macro that generates a `BusDebug` implementation for a struct.
///
/// Annotate fields with:
/// - `#[debug_device("Name")]` — field implements `Debuggable`, listed in `devices()`
/// - `#[debug_cpu("Name", read = "method", write = "method")]` — field implements
///   `DebugCpu`, listed in both `devices()` AND `cpus()`. The `read`/`write` name
///   existing `&self` / `&mut self` methods on the struct for side-effect-free memory access.
///
/// CPU index assignment is positional: first `#[debug_cpu]` is index 0, etc.
#[proc_macro_derive(BusDebug, attributes(debug_device, debug_cpu))]
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

    let mut device_entries = Vec::new(); // (name, field_ident) for all annotated fields
    let mut cpu_entries = Vec::new(); // (name, field_ident, read_method, write_method)

    for field in fields {
        let field_ident = field.ident.as_ref().expect("named field");

        for attr in &field.attrs {
            if attr.path().is_ident("debug_device") {
                // #[debug_device("Name")]
                let name: syn::LitStr = attr
                    .parse_args()
                    .expect("debug_device expects a string literal: #[debug_device(\"Name\")]");
                device_entries.push((name, field_ident.clone()));
            } else if attr.path().is_ident("debug_cpu") {
                // #[debug_cpu("Name", read = "method", write = "method")]
                let args: CpuArgs = attr
                    .parse_args()
                    .expect("debug_cpu expects: (\"Name\", read = \"method\", write = \"method\")");
                // CPUs appear in both devices() and cpus()
                device_entries.push((args.name.clone(), field_ident.clone()));
                cpu_entries.push((args.name, field_ident.clone(), args.read, args.write));
            }
        }
    }

    // Generate devices() body
    let device_items = device_entries.iter().map(|(name, ident)| {
        quote! { (#name, &self.#ident as &dyn phosphor_core::core::debug::Debuggable) }
    });

    // Generate cpus() body
    let cpu_items = cpu_entries.iter().map(|(name, ident, _, _)| {
        quote! { (#name, &self.#ident as &dyn phosphor_core::core::debug::DebugCpu) }
    });

    // Generate read() match arms
    let read_arms = cpu_entries
        .iter()
        .enumerate()
        .map(|(i, (_, _, read_method, _))| {
            let read_ident = syn::Ident::new(read_method.value().as_str(), read_method.span());
            let idx = i;
            quote! { #idx => self.#read_ident(addr) }
        });

    // Generate write() match arms
    let write_arms = cpu_entries
        .iter()
        .enumerate()
        .map(|(i, (_, _, _, write_method))| {
            let write_ident = syn::Ident::new(write_method.value().as_str(), write_method.span());
            let idx = i;
            quote! { #idx => self.#write_ident(addr, data) }
        });

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
        }
    };

    TokenStream::from(expanded)
}

/// Parsed arguments for `#[debug_cpu("Name", read = "method", write = "method")]`.
struct CpuArgs {
    name: syn::LitStr,
    read: syn::LitStr,
    write: syn::LitStr,
}

impl syn::parse::Parse for CpuArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: syn::LitStr = input.parse()?;
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

        Ok(CpuArgs {
            name,
            read: read.ok_or_else(|| input.error("missing `read = \"method\"` attribute"))?,
            write: write.ok_or_else(|| input.error("missing `write = \"method\"` attribute"))?,
        })
    }
}
