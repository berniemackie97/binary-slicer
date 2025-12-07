//! Disassembly and analysis backends.
//!
//! This module will provide adapters for tools like:
//! - Capstone (disassembly)
//! - rizin (analysis, symbols, xrefs)
//! - IDA / Ghidra import/export
//!
//! For now, it only defines placeholder traits so the rest of the system
//! can be designed against stable interfaces.

/// Placeholder trait for a disassembly backend.
///
/// In later steps, this will be implemented using Capstone, rizin, etc.
pub trait DisassemblyBackend: Send + Sync {
    /// Returns a human-readable name for the backend.
    fn name(&self) -> &str;
}
