/// PIA 6820 Peripheral Interface Adapter
pub struct Pia6820 {
    #[allow(dead_code)]
    port_a: u8,
    #[allow(dead_code)]
    port_b: u8,
}

impl Pia6820 {
    pub fn new() -> Self {
        Self {
            port_a: 0,
            port_b: 0,
        }
    }
    
    pub fn dma_requested(&self) -> bool {
        false
    }
    
    pub fn do_dma_cycle<B: crate::core::Bus + ?Sized>(&mut self, _bus: &mut B) {
        // Placeholder
    }
}

impl Default for Pia6820 {
    fn default() -> Self {
        Self::new()
    }
}
