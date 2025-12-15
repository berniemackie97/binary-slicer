use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use capstone::{arch, prelude::*, Capstone, InsnGroupId};
use goblin::{elf, mach, pe, Object};

use crate::services::analysis::{
    AnalysisBackend, AnalysisError, AnalysisRequest, AnalysisResult, BlockEdge, BlockEdgeKind,
    CallEdge, EvidenceRecord, FunctionRecord,
};

pub struct CapstoneBackend;

#[derive(Debug, Clone)]
struct SymbolInfo {
    name: String,
    address: u64,
    size: Option<u64>,
    file_range: Option<(usize, usize)>,
}

#[derive(Debug, Clone)]
struct SectionRange {
    name: String,
    start: u64,
    end: u64,
    file_offset: Option<usize>,
    size: Option<usize>,
}

fn capstone_version() -> Option<String> {
    let (major, minor) = Capstone::lib_version();
    Some(format!("{major}.{minor}"))
}

fn capstone_arch_from_hint(hint: Option<&str>) -> Option<String> {
    hint.map(|h| h.to_lowercase())
}

fn capstone_arch_from_object(bytes: &[u8]) -> Option<String> {
    if let Ok(obj) = Object::parse(bytes) {
        match obj {
            Object::Elf(elf) => match elf.header.e_machine {
                elf::header::EM_X86_64 => Some("x86_64".into()),
                elf::header::EM_386 => Some("x86".into()),
                elf::header::EM_AARCH64 => Some("arm64".into()),
                elf::header::EM_ARM => Some("arm".into()),
                _ => None,
            },
            Object::PE(pe) => match pe.header.coff_header.machine {
                pe::header::COFF_MACHINE_X86 => Some("x86".into()),
                pe::header::COFF_MACHINE_X86_64 => Some("x86_64".into()),
                pe::header::COFF_MACHINE_ARM => Some("arm".into()),
                pe::header::COFF_MACHINE_ARM64 => Some("arm64".into()),
                _ => None,
            },
            Object::Mach(mach::Mach::Binary(bin)) => match bin.header.cputype() {
                mach::cputype::CPU_TYPE_X86 => Some("x86".into()),
                mach::cputype::CPU_TYPE_X86_64 => Some("x86_64".into()),
                mach::cputype::CPU_TYPE_ARM => Some("arm".into()),
                mach::cputype::CPU_TYPE_ARM64 => Some("arm64".into()),
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    }
}

fn make_cs(arch: &str) -> Result<Capstone, AnalysisError> {
    match arch {
        "x86_64" | "amd64" => Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode64)
            .detail(true)
            .build()
            .map_err(|e| AnalysisError::Backend(format!("capstone init failed: {e}"))),
        "x86" | "i386" => Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode32)
            .detail(true)
            .build()
            .map_err(|e| AnalysisError::Backend(format!("capstone init failed: {e}"))),
        "arm" | "armv7" => Capstone::new()
            .arm()
            .mode(arch::arm::ArchMode::Arm)
            .detail(true)
            .build()
            .map_err(|e| AnalysisError::Backend(format!("capstone init failed: {e}"))),
        "arm64" | "aarch64" => Capstone::new()
            .arm64()
            .mode(arch::arm64::ArchMode::Arm)
            .detail(true)
            .build()
            .map_err(|e| AnalysisError::Backend(format!("capstone init failed: {e}"))),
        "riscv" | "riscv64" => Capstone::new()
            .riscv()
            .mode(arch::riscv::ArchMode::RiscV64)
            .detail(true)
            .build()
            .map_err(|e| AnalysisError::Backend(format!("capstone init failed: {e}"))),
        "riscv32" => Capstone::new()
            .riscv()
            .mode(arch::riscv::ArchMode::RiscV32)
            .detail(true)
            .build()
            .map_err(|e| AnalysisError::Backend(format!("capstone init failed: {e}"))),
        "ppc" | "powerpc" | "ppc64" => Capstone::new()
            .ppc()
            .mode(arch::ppc::ArchMode::Mode64)
            .detail(true)
            .build()
            .map_err(|e| AnalysisError::Backend(format!("capstone init failed: {e}"))),
        other => {
            Capstone::new().x86().mode(arch::x86::ArchMode::Mode64).detail(true).build().map_err(
                |e| AnalysisError::Backend(format!("capstone init failed for {other}: {e}")),
            )
        }
    }
}

fn section_range_to_file(
    addr: u64,
    size: Option<u64>,
    sec_addr: u64,
    sec_size: u64,
    sec_offset: u64,
    bytes_len: usize,
) -> Option<(usize, usize)> {
    if addr < sec_addr || addr >= sec_addr + sec_size {
        return None;
    }
    let offset_in_section = addr.saturating_sub(sec_addr);
    let start = sec_offset.saturating_add(offset_in_section);
    if start as usize >= bytes_len {
        return None;
    }
    let available = sec_size.saturating_sub(offset_in_section);
    let length = size.unwrap_or(available).min(available);
    let end = start.saturating_add(length);
    let end = end.min(bytes_len as u64);
    if end <= start {
        None
    } else {
        Some((start as usize, end as usize))
    }
}

fn elf_symbols(elf: &elf::Elf, bytes_len: usize) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();
    for sym in &elf.syms {
        if sym.is_function()
            && sym.st_value > 0
            && sym.st_shndx != elf::section_header::SHN_UNDEF as usize
        {
            let name = elf.strtab.get_at(sym.st_name).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let size = if sym.st_size > 0 { Some(sym.st_size) } else { None };
            let file_range = elf.section_headers.get(sym.st_shndx).and_then(|shdr| {
                section_range_to_file(
                    sym.st_value,
                    size,
                    shdr.sh_addr,
                    shdr.sh_size,
                    shdr.sh_offset,
                    bytes_len,
                )
            });
            symbols.push(SymbolInfo { name, address: sym.st_value, size, file_range });
        }
    }
    symbols
}

fn mach_symbols(bin: &mach::MachO, bytes_len: usize) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();
    for sym in bin.symbols() {
        let Ok((name, nlist)) = sym else { continue };
        if nlist.n_value == 0 {
            continue;
        }
        let name = name.trim_start_matches('_').to_string();
        if name.is_empty() {
            continue;
        }
        symbols.push(SymbolInfo { name, address: nlist.n_value, size: None, file_range: None });
    }

    // Best-effort: map to sections to recover file slices when possible.
    let mut mapped = Vec::new();
    for (sec, _) in bin.segments.sections().flatten().filter_map(Result::ok) {
        mapped.push(sec);
    }
    for sym in symbols.iter_mut() {
        for sec in &mapped {
            if let Some(range) = section_range_to_file(
                sym.address,
                sym.size,
                sec.addr,
                sec.size,
                sec.offset.into(),
                bytes_len,
            ) {
                sym.file_range = Some(range);
                break;
            }
        }
    }
    symbols
}

fn pe_symbols(pe: &pe::PE, _bytes_len: usize) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();
    for exp in &pe.exports {
        if exp.rva == 0 {
            continue;
        }
        let name = exp.name.unwrap_or_default().to_string();
        if name.is_empty() {
            continue;
        }
        let mut file_range = None;
        for sec in &pe.sections {
            let start = sec.virtual_address as u64;
            let size = if sec.virtual_size == 0 {
                sec.size_of_raw_data as u64
            } else {
                sec.virtual_size as u64
            };
            if (exp.rva as u64) >= start && (exp.rva as u64) < start + size {
                let offset = sec.pointer_to_raw_data as u64 + (exp.rva as u64 - start);
                let available = size.saturating_sub(exp.rva as u64 - start);
                file_range = Some((offset as usize, offset.saturating_add(available) as usize));
                break;
            }
        }

        symbols.push(SymbolInfo { name, address: exp.rva as u64, size: None, file_range });
    }
    symbols
}

fn extract_symbols(bytes: &[u8]) -> Vec<SymbolInfo> {
    match Object::parse(bytes) {
        Ok(Object::Elf(elf)) => elf_symbols(&elf, bytes.len()),
        Ok(Object::PE(pe)) => pe_symbols(&pe, bytes.len()),
        Ok(Object::Mach(mach::Mach::Binary(bin))) => mach_symbols(&bin, bytes.len()),
        _ => Vec::new(),
    }
}

fn collect_sections(bytes: &[u8]) -> Vec<SectionRange> {
    match Object::parse(bytes) {
        Ok(Object::Elf(elf)) => elf
            .section_headers
            .iter()
            .map(|sh| SectionRange {
                name: elf.strtab.get_at(sh.sh_name).unwrap_or("").to_string(),
                start: sh.sh_addr,
                end: sh.sh_addr.saturating_add(sh.sh_size),
                file_offset: Some(sh.sh_offset as usize),
                size: Some(sh.sh_size as usize),
            })
            .collect(),
        Ok(Object::PE(pe)) => pe
            .sections
            .iter()
            .map(|sec| SectionRange {
                name: sec.name().unwrap_or_default().to_string(),
                start: sec.virtual_address as u64,
                end: sec.virtual_address as u64 + sec.virtual_size as u64,
                file_offset: Some(sec.pointer_to_raw_data as usize),
                size: Some(sec.size_of_raw_data as usize),
            })
            .collect(),
        Ok(Object::Mach(mach::Mach::Binary(bin))) => bin
            .segments
            .sections()
            .flatten()
            .filter_map(|res| res.ok())
            .map(|(sec, _)| SectionRange {
                name: sec.name().unwrap_or("").to_string(),
                start: sec.addr,
                end: sec.addr.saturating_add(sec.size),
                file_offset: Some(sec.offset as usize),
                size: Some(sec.size as usize),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn decode_call_target(detail: &capstone::InsnDetail) -> Option<u64> {
    detail.arch_detail().operands().iter().find_map(|op| match op {
        capstone::arch::ArchOperand::X86Operand(op) => {
            if let capstone::arch::x86::X86OperandType::Imm(imm) = op.op_type {
                Some(imm as u64)
            } else {
                None
            }
        }
        capstone::arch::ArchOperand::ArmOperand(op) => {
            if let capstone::arch::arm::ArmOperandType::Imm(imm) = op.op_type {
                Some(imm as u64)
            } else {
                None
            }
        }
        capstone::arch::ArchOperand::Arm64Operand(op) => {
            if let capstone::arch::arm64::Arm64OperandType::Imm(imm) = op.op_type {
                Some(imm as u64)
            } else {
                None
            }
        }
        _ => None,
    })
}

fn operand_evidence(
    detail: &capstone::InsnDetail,
    sections: &[SectionRange],
    bytes: &[u8],
    address: u64,
    evidence: &mut Vec<EvidenceRecord>,
) {
    let preview_for = |sec: &SectionRange, imm: u64| -> Option<String> {
        let file_off = sec.file_offset?;
        let size = sec.size?;
        if imm < sec.start || imm >= sec.end {
            return None;
        }
        let offset_in_sec = (imm - sec.start) as usize;
        if offset_in_sec >= size {
            return None;
        }
        let start = file_off.saturating_add(offset_in_sec);
        if start >= bytes.len() {
            return None;
        }
        let end = (start + 16).min(bytes.len());
        let slice = &bytes[start..end];
        let mut s = String::new();
        for b in slice {
            let ch = *b as char;
            if ch.is_ascii_graphic() || ch == ' ' {
                s.push(ch);
            } else {
                s.push('.');
            }
        }
        Some(s)
    };

    for op in detail.arch_detail().operands() {
        match op {
            capstone::arch::ArchOperand::X86Operand(op) => match &op.op_type {
                capstone::arch::x86::X86OperandType::Imm(imm) => {
                    if let Some(sec) =
                        sections.iter().find(|s| (*imm as u64) >= s.start && (*imm as u64) < s.end)
                    {
                        if let Some(preview) = preview_for(sec, *imm as u64) {
                            evidence.push(EvidenceRecord {
                                address,
                                description: format!(
                                    "xref imm 0x{imm:X} -> section {} (0x{:X}-0x{:X}) preview=\"{preview}\"",
                                    sec.name, sec.start, sec.end
                                ),
                                kind: None,
                            });
                        } else {
                            evidence.push(EvidenceRecord {
                                address,
                                description: format!(
                                    "xref imm 0x{imm:X} -> section {} (0x{:X}-0x{:X})",
                                    sec.name, sec.start, sec.end
                                ),
                                kind: None,
                            });
                        }
                    }
                }
                capstone::arch::x86::X86OperandType::Reg(reg) => {
                    evidence.push(EvidenceRecord {
                        address,
                        description: format!("reg operand {:?}", reg.0),
                        kind: None,
                    });
                }
                capstone::arch::x86::X86OperandType::Mem(mem) => {
                    let disp = mem.disp();
                    evidence.push(EvidenceRecord {
                        address,
                        description: format!(
                            "mem operand base={:?} index={:?} scale={} disp=0x{disp:X}",
                            mem.base().0,
                            mem.index().0,
                            mem.scale()
                        ),
                        kind: None,
                    });
                }
                _ => {}
            },
            capstone::arch::ArchOperand::ArmOperand(op) => {
                if let capstone::arch::arm::ArmOperandType::Imm(imm) = op.op_type {
                    if let Some(sec) =
                        sections.iter().find(|s| (imm as u64) >= s.start && (imm as u64) < s.end)
                    {
                        if let Some(preview) = preview_for(sec, imm as u64) {
                            evidence.push(EvidenceRecord {
                                address,
                                description: format!(
                                    "xref imm 0x{imm:X} -> section {} (0x{:X}-0x{:X}) preview=\"{preview}\"",
                                    sec.name, sec.start, sec.end
                                ),
                                kind: None,
                            });
                        } else {
                            evidence.push(EvidenceRecord {
                                address,
                                description: format!(
                                    "xref imm 0x{imm:X} -> section {} (0x{:X}-0x{:X})",
                                    sec.name, sec.start, sec.end
                                ),
                                kind: None,
                            });
                        }
                    }
                }
                if let capstone::arch::arm::ArmOperandType::Reg(reg) = op.op_type {
                    evidence.push(EvidenceRecord {
                        address,
                        description: format!("reg operand {:?}", reg.0),
                        kind: None,
                    });
                }
            }
            capstone::arch::ArchOperand::Arm64Operand(op) => match op.op_type {
                capstone::arch::arm64::Arm64OperandType::Imm(imm) => {
                    if let Some(sec) =
                        sections.iter().find(|s| (imm as u64) >= s.start && (imm as u64) < s.end)
                    {
                        if let Some(preview) = preview_for(sec, imm as u64) {
                            evidence.push(EvidenceRecord {
                                    address,
                                    description: format!(
                                        "xref imm 0x{imm:X} -> section {} (0x{:X}-0x{:X}) preview=\"{preview}\"",
                                        sec.name, sec.start, sec.end
                                    ),
                                    kind: None,
                                });
                        } else {
                            evidence.push(EvidenceRecord {
                                address,
                                description: format!(
                                    "xref imm 0x{imm:X} -> section {} (0x{:X}-0x{:X})",
                                    sec.name, sec.start, sec.end
                                ),
                                kind: None,
                            });
                        }
                    }
                }
                capstone::arch::arm64::Arm64OperandType::Reg(reg) => {
                    evidence.push(EvidenceRecord {
                        address,
                        description: format!("reg operand {:?}", reg.0),
                        kind: None,
                    });
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl CapstoneBackend {
    fn load_bytes(path: &PathBuf) -> Result<Vec<u8>, AnalysisError> {
        fs::read(path).map_err(|_| AnalysisError::MissingBinary(path.clone()))
    }
}

impl AnalysisBackend for CapstoneBackend {
    fn analyze(&self, request: &AnalysisRequest) -> Result<AnalysisResult, AnalysisError> {
        let bytes = Self::load_bytes(&request.binary_path)?;
        let backend_version = capstone_version();
        if bytes.is_empty() {
            return Ok(AnalysisResult {
                functions: vec![],
                call_edges: vec![],
                evidence: vec![],
                basic_blocks: vec![],
                roots: request.roots.clone(),
                backend_version,
                backend_path: None,
            });
        }

        let arch = capstone_arch_from_hint(request.arch.as_deref())
            .or_else(|| capstone_arch_from_object(&bytes))
            .unwrap_or_else(|| "x86_64".to_string());
        let cs = make_cs(&arch)?;

        let max_instructions = request.options.max_instructions.unwrap_or(2048);
        let mut evidence = Vec::new();
        let mut call_edges = Vec::new();
        let mut functions = Vec::new();
        let mut basic_blocks = Vec::new();
        let section_ranges = collect_sections(&bytes);

        let symbols = extract_symbols(&bytes);
        for sym in symbols {
            if let Some((start, end)) = sym.file_range {
                let slice = &bytes[start..end];
                if let Ok(insns) =
                    cs.disasm_count(slice, sym.address, max_instructions.saturating_sub(1))
                {
                    let mut current_block_start = insns.iter().next().map(|i| i.address());
                    let mut current_block_len: u32 = 0;
                    let mut successors: Vec<BlockEdge> = Vec::new();

                    for (idx, i) in insns.iter().take(64).enumerate() {
                        if evidence.len() >= 128 {
                            break;
                        }
                        evidence.push(EvidenceRecord {
                            address: i.address(),
                            description: format!(
                                "{} {}",
                                i.mnemonic().unwrap_or(""),
                                i.op_str().unwrap_or("")
                            )
                            .trim()
                            .to_string(),
                            kind: None,
                        });
                        current_block_len += 1;

                        if let Ok(detail) = cs.insn_detail(i) {
                            let is_call = detail.groups().iter().any(|g| {
                                *g == InsnGroupId(capstone::InsnGroupType::CS_GRP_CALL as u8)
                            });
                            let is_jump = detail.groups().iter().any(|g| {
                                *g == InsnGroupId(capstone::InsnGroupType::CS_GRP_JUMP as u8)
                            });
                            let is_ret = detail.groups().iter().any(|g| {
                                *g == InsnGroupId(capstone::InsnGroupType::CS_GRP_RET as u8)
                            });

                            if is_call {
                                let indirect = detail.arch_detail().operands().iter().any(|op| {
                                    !matches!(
                                        op,
                                        capstone::arch::ArchOperand::X86Operand(x)
                                            if matches!(x.op_type, capstone::arch::x86::X86OperandType::Imm(_))
                                    ) && !matches!(
                                        op,
                                        capstone::arch::ArchOperand::ArmOperand(x)
                                            if matches!(x.op_type, capstone::arch::arm::ArmOperandType::Imm(_))
                                    ) && !matches!(
                                        op,
                                        capstone::arch::ArchOperand::Arm64Operand(x)
                                            if matches!(x.op_type, capstone::arch::arm64::Arm64OperandType::Imm(_))
                                    )
                                });
                                if let Some(target) = decode_call_target(&detail) {
                                    call_edges.push(CallEdge {
                                        from: i.address(),
                                        to: target,
                                        is_cross_slice: false,
                                    });
                                    successors.push(BlockEdge {
                                        target,
                                        kind: if indirect {
                                            BlockEdgeKind::IndirectCall
                                        } else {
                                            BlockEdgeKind::Call
                                        },
                                    });
                                    evidence.push(EvidenceRecord {
                                        address: i.address(),
                                        description: format!(
                                            "call_edge 0x{:X} -> 0x{:X}",
                                            i.address(),
                                            target
                                        ),
                                        kind: None,
                                    });
                                }
                            } else if is_jump {
                                let mnemonic = i.mnemonic().unwrap_or("").to_lowercase();
                                let conditional = mnemonic.starts_with('j')
                                    && mnemonic != "jmp"
                                    && mnemonic != "jr";
                                let indirect = detail.arch_detail().operands().iter().any(|op| {
                                    !matches!(
                                        op,
                                        capstone::arch::ArchOperand::X86Operand(x)
                                            if matches!(x.op_type, capstone::arch::x86::X86OperandType::Imm(_))
                                    ) && !matches!(
                                        op,
                                        capstone::arch::ArchOperand::ArmOperand(x)
                                            if matches!(x.op_type, capstone::arch::arm::ArmOperandType::Imm(_))
                                    ) && !matches!(
                                        op,
                                        capstone::arch::ArchOperand::Arm64Operand(x)
                                            if matches!(x.op_type, capstone::arch::arm64::Arm64OperandType::Imm(_))
                                    )
                                });
                                if let Some(target) = decode_call_target(&detail) {
                                    successors.push(BlockEdge {
                                        target,
                                        kind: if indirect {
                                            BlockEdgeKind::IndirectJump
                                        } else if conditional {
                                            BlockEdgeKind::ConditionalJump
                                        } else {
                                            BlockEdgeKind::Jump
                                        },
                                    });
                                }
                            }

                            operand_evidence(
                                &detail,
                                &section_ranges,
                                &bytes,
                                i.address(),
                                &mut evidence,
                            );

                            let is_last = idx + 1 == insns.len();
                            if is_call || is_jump || is_ret || is_last {
                                if !is_ret && !is_jump {
                                    if let Some(next) = insns.get(idx + 1) {
                                        successors.push(BlockEdge {
                                            target: next.address(),
                                            kind: BlockEdgeKind::Fallthrough,
                                        });
                                    }
                                }
                                if let Some(start_addr) = current_block_start {
                                    basic_blocks.push(crate::services::analysis::BasicBlock {
                                        start: start_addr,
                                        len: current_block_len.max(1),
                                        successors: successors.clone(),
                                    });
                                }
                                current_block_start =
                                    i.address().checked_add(i.bytes().len() as u64);
                                current_block_len = 0;
                                successors.clear();
                            }
                        }
                    }
                }
            }

            functions.push(FunctionRecord {
                address: sym.address,
                name: Some(sym.name.clone()),
                size: sym.size.map(|s| s as u32),
                in_slice: request.roots.iter().any(|r| r == &sym.name),
                is_boundary: false,
            });
        }

        if evidence.is_empty() {
            if let Ok(insns) = cs.disasm_count(&bytes, 0, max_instructions) {
                let mut current_block_start = insns.iter().next().map(|i| i.address());
                let mut current_block_len: u32 = 0;
                for i in insns.iter().take(128) {
                    evidence.push(EvidenceRecord {
                        address: i.address(),
                        description: format!(
                            "{} {}",
                            i.mnemonic().unwrap_or(""),
                            i.op_str().unwrap_or("")
                        )
                        .trim()
                        .to_string(),
                        kind: None,
                    });
                    current_block_len += 1;
                    if let Ok(detail) = cs.insn_detail(i) {
                        let is_block_term = detail.groups().iter().any(|g| {
                            *g == InsnGroupId(capstone::InsnGroupType::CS_GRP_CALL as u8)
                                || *g == InsnGroupId(capstone::InsnGroupType::CS_GRP_JUMP as u8)
                                || *g == InsnGroupId(capstone::InsnGroupType::CS_GRP_RET as u8)
                        });
                        if is_block_term {
                            if let Some(start_addr) = current_block_start {
                                evidence.push(EvidenceRecord {
                                    address: start_addr,
                                    description: format!(
                                        "basic_block start=0x{start_addr:016X} len={current_block_len}"
                                    ),
                                    kind: None,
                                });
                            }
                            current_block_start = i.address().checked_add(i.bytes().len() as u64);
                            current_block_len = 0;
                        }
                        if detail
                            .groups()
                            .contains(&InsnGroupId(capstone::InsnGroupType::CS_GRP_CALL as u8))
                        {
                            if let Some(target) = decode_call_target(&detail) {
                                call_edges.push(CallEdge {
                                    from: i.address(),
                                    to: target,
                                    is_cross_slice: false,
                                });
                                evidence.push(EvidenceRecord {
                                    address: i.address(),
                                    description: format!(
                                        "call_edge 0x{:X} -> 0x{:X}",
                                        i.address(),
                                        target
                                    ),
                                    kind: None,
                                });
                            }
                        }
                    }
                }
                if let Some(start_addr) = current_block_start {
                    basic_blocks.push(crate::services::analysis::BasicBlock {
                        start: start_addr,
                        len: current_block_len.max(1),
                        successors: Vec::new(),
                    });
                }
            }
        }

        if functions.is_empty() {
            let mut seen = HashSet::new();
            for edge in &call_edges {
                if edge.to > 0 && seen.insert(edge.to) {
                    functions.push(FunctionRecord {
                        address: edge.to,
                        name: Some(format!("sub_{:X}", edge.to)),
                        size: None,
                        in_slice: false,
                        is_boundary: false,
                    });
                }
            }
        }

        if functions.is_empty() {
            // Fallback: honor roots so callers still get deterministic results.
            functions = request
                .roots
                .iter()
                .enumerate()
                .map(|(idx, name)| FunctionRecord {
                    address: 0x1000 + idx as u64,
                    name: Some(name.clone()),
                    size: None,
                    in_slice: true,
                    is_boundary: false,
                })
                .collect();
        }

        Ok(AnalysisResult {
            functions,
            call_edges,
            evidence,
            basic_blocks,
            roots: request.roots.clone(),
            backend_version,
            backend_path: None,
        })
    }

    fn name(&self) -> &'static str {
        "capstone"
    }
}
