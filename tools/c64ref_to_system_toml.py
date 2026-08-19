#!/usr/bin/env python3
"""
Convert Commodore 64 KERNAL labels and comments from the c64ref repository
(https://www.pagetable.com/c64ref/kernal/) into TOML format compatible with
Regenerator 2000's system definition files (e.g., system-commodore_64.toml).

Sources in c64ref:
  - src/kernal/: 11 commentary sources for KERNAL API jump table ($FF81-$FFF3)
  - src/c64disasm/: Official Commodore & community KERNAL ROM disassemblies ($E000-$FFFF)
  - src/c64mem/symbols.txt: Zero-page / lower-page memory symbols ($0000-$03FF)

Usage:
  # View help
  python3 tools/c64ref_to_system_toml.py --help

  # Extract KERNAL Jump Table ($FF81-$FFF3) + Jump Targets ($E500-$FDA3) to stdout
  python3 tools/c64ref_to_system_toml.py --mode api

  # Extract Full KERNAL Disassembly Labels ($E000-$FFFF) from Commodore source
  python3 tools/c64ref_to_system_toml.py --mode all

  # Directly update system-commodore_64.toml in place
  python3 tools/c64ref_to_system_toml.py --mode all --update-toml crates/regenerator2000-core/assets/systems/system-commodore_64.toml
"""

from __future__ import annotations

import argparse
import collections
import os
import re
import sys
from pathlib import Path
from typing import Optional, Tuple

# 6502 Instruction Mnemonics and Assembler Directives to filter out false labels
OPCODES: set[str] = {
    "ADC", "AND", "ASL", "BCC", "BCS", "BEQ", "BIT", "BMI", "BNE", "BPL", "BRK",
    "BVC", "BVS", "CLC", "CLD", "CLI", "CLV", "CMP", "CPX", "CPY", "DEC", "DEX",
    "DEY", "EOR", "INC", "INX", "INY", "JMP", "JSR", "LDA", "LDX", "LDY", "LSR",
    "NOP", "ORA", "PHA", "PHP", "PLA", "PLP", "ROL", "ROR", "RTI", "RTS", "SBC",
    "SEC", "SED", "SEI", "STA", "STX", "STY", "TAX", "TAY", "TSX", "TXA", "TXS",
    "TYA", "BYTE", "WORD", "DBYTE", "LIB", "END", "PAGE", "SKIP", "SKI", "OPT",
    "TITLE", "FILE", "ALIGN", "DS", "EQU", "ORG", "BYT", "DBYT", "*="
}


def is_valid_label(name: str) -> bool:
    """Check if a token is a valid, non-local, non-directive assembly label."""
    if not name or name.startswith("."):
        return False
    if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", name):
        return False
    if name.upper() in OPCODES:
        return False
    return True

# Standard 39 KERNAL Jump Table routines ($FF81 - $FFF3) and their internal jump targets on C64
# (Derived from KERNAL ROM 901227-03 and Dan Heeb / Tool Kit references)
KERNAL_JUMP_TARGETS: dict[int, int] = {
    0xFF81: 0xFF5B,  # CINT -> CINTj (screen editor & VIC setup)
    0xFF84: 0xFDA3,  # IOINIT -> IOINITj (initialize CIA & IRQ)
    0xFF87: 0xFD50,  # RAMTAS -> RAMTASj (RAM test & search RAM end)
    0xFF8A: 0xFD15,  # RESTOR -> RESTORj (restore default I/O vectors)
    0xFF8D: 0xFD1A,  # VECTOR -> VECTORj (read/set I/O vectors)
    0xFF90: 0xFE18,  # SETMSG -> SETMSGj (enable/disable KERNAL messages)
    0xFF93: 0xEDB9,  # SECOND -> SECONDj (send secondary addr after listen)
    0xFF96: 0xEDC7,  # TKSA -> TKSAj (send secondary addr after talk)
    0xFF99: 0xFE25,  # MEMTOP -> MEMTOPj (read/set top of memory)
    0xFF9C: 0xFE34,  # MEMBOT -> MEMBOTj (read/set bottom of memory)
    0xFF9F: 0xEA87,  # SCNKEY -> SCNKEYj (scan keyboard)
    0xFFA2: 0xFE21,  # SETTMO -> SETTMOj (set IEEE timeout)
    0xFFA5: 0xEE13,  # ACPTR -> ACPTRj (input byte from serial)
    0xFFA8: 0xEDDD,  # CIOUT -> CIOUTj (output byte to serial)
    0xFFAB: 0xEDEF,  # UNTLK -> UNTLKj (untalk all serial devices)
    0xFFAE: 0xEDFE,  # UNLSN -> UNLSNj (unlisten all serial devices)
    0xFFB1: 0xED0C,  # LISTEN -> LISTENj (make serial device listen)
    0xFFB4: 0xED09,  # TALK -> TALKj (make serial device talk)
    0xFFB7: 0xFE07,  # READST -> READSTj (read I/O status byte)
    0xFFBA: 0xFE00,  # SETLFS -> SETLFSj (set file parameters)
    0xFFBD: 0xFDF9,  # SETNAM -> SETNAMj (set file name)
    0xFFC0: 0xF34A,  # OPEN -> OPENj (open logical file, indirect via $031A)
    0xFFC3: 0xF291,  # CLOSE -> CLOSEj (close a logical file, indirect via $031C)
    0xFFC6: 0xF20E,  # CHKIN -> CHKINj (open channel for input, indirect via $031E)
    0xFFC9: 0xF250,  # CHKOUT -> CHKOUTj (open channel for output, indirect via $0320)
    0xFFCC: 0xF333,  # CLRCHN -> CLRCHNj (restore default devices, indirect via $0322)
    0xFFCF: 0xF157,  # CHRIN -> CHRINj (input character, indirect via $0324)
    0xFFD2: 0xF1CA,  # CHROUT -> CHROUTj (output character, indirect via $0326)
    0xFFD5: 0xF49E,  # LOAD -> LOADj (load file, vector via $0330)
    0xFFD8: 0xF5DD,  # SAVE -> SAVEj (save file, vector via $0332)
    0xFFDB: 0xF6E4,  # SETTIM -> SETTIMj (set jiffy clock)
    0xFFDE: 0xF6DD,  # RDTIM -> RDTIMj (read jiffy clock)
    0xFFE1: 0xF6ED,  # STOP -> STOPj (check stop key, indirect via $0328)
    0xFFE4: 0xF13E,  # GETIN -> GETINj (get a byte from channel, indirect via $032A)
    0xFFE7: 0xF32F,  # CLALL -> CLALLj (close or abort all files, indirect via $032C)
    0xFFEA: 0xF69B,  # UDTIM -> UDTIMj (update jiffy clock)
    0xFFED: 0xE505,  # SCREEN -> SCREENj (return screen size)
    0xFFF0: 0xE50A,  # PLOT -> PLOTj (read/set cursor position)
    0xFFF3: 0xE500,  # IOBASE -> IOBASEj (return address of I/O devices)
}

# Standard hardware and interrupt vector labels
HARDWARE_VECTORS: dict[int, str] = {
    0xFCE2: "KERNAL_RESET_VECTOR",
    0xFE43: "KERNAL_NMI_VECTOR",
    0xFF48: "KERNAL_IRQ_BRK_VECTOR",
    0xFFFA: "HW_VEC_NMI_LO",
    0xFFFB: "HW_VEC_NMI_HI",
    0xFFFC: "HW_VEC_RESET_LO",
    0xFFFD: "HW_VEC_RESET_HI",
    0xFFFE: "HW_VEC_IRQ_LO",
    0xFFFF: "HW_VEC_IRQ_HI",
}

# Commentary source file mapping in src/kernal/
COMMENTARY_FILES: dict[str, str] = {
    "prg": "kernal_prg.txt",          # Programmer's Reference Guide
    "dh": "kernal_dh.txt",            # Tool Kit (Dan Heeb)
    "mlr": "kernal_mlr.txt",          # Machine Language Routines
    "mapc64": "kernal_mapc64.txt",    # Mapping the Commodore 64
    "128intern": "kernal_128intern.txt", # Commodore 128 intern
    "ld": "kernal_ld.txt",            # Lee Davison
    "pm": "kernal_pm.txt",            # Cracking The Kernal (Peter Marcotty)
    "ct": "kernal_ct.txt",            # Craig Taylor
    "sta": "kernal_sta.txt",          # Joe Forster/STA
    "fk": "kernal_fk.txt",            # Frank Kontros (used in standard system TOML)
    "64intern": "kernal_64intern.txt",# 64 intern
}

# Disassembly source files in src/c64disasm/
DISASM_FILES: dict[str, str] = {
    "cbm": "c64disasm_cbm.txt",       # Official Commodore KERNAL Source (901227-03)
    "en": "c64disasm_en.txt",         # Lee Davison English Disassembly
    "de": "c64disasm_de.txt",         # Data Becker German Disassembly
    "mm": "c64disasm_mm.txt",         # Marko Mäkelä Disassembly
    "mn": "c64disasm_mn.txt",         # Magnus Nyman (JiffyDOS) Disassembly
}


class KernalEntry:
    """Represents a single KERNAL API routine entry."""

    def __init__(self, address: int, canonical_symbol: str) -> None:
        self.address = address
        self.canonical_symbol = canonical_symbol
        self.symbols_by_source: dict[str, str] = {}
        self.summaries_by_source: dict[str, str] = {}
        self.details_by_source: dict[str, list[str]] = {}

    def get_summary(self, preferred_source: str = "fk") -> str:
        """Return summary from preferred source, or first non-empty summary."""
        if preferred_source in self.summaries_by_source and self.summaries_by_source[preferred_source]:
            return self.summaries_by_source[preferred_source]
        for src in ["fk", "prg", "mapc64", "dh", "sta", "pm", "ct", "ld"]:
            if src in self.summaries_by_source and self.summaries_by_source[src]:
                return self.summaries_by_source[src]
        return ""


def parse_kernal_api_dir(kernal_dir: Path) -> dict[int, KernalEntry]:
    """Parse all text files in c64ref/src/kernal/ to build consensus symbols and descriptions."""
    entries: dict[int, KernalEntry] = {}
    all_symbols_for_addr: dict[int, list[str]] = collections.defaultdict(list)

    # First pass: read all sources
    raw_sources: dict[str, dict[int, tuple[str, str, list[str]]]] = {}
    for src_key, filename in COMMENTARY_FILES.items():
        filepath = kernal_dir / filename
        if not filepath.is_file():
            continue

        with open(filepath, encoding="utf-8", errors="ignore") as f:
            lines = [line.rstrip() for line in f]

        calls_raw: list[list[str]] = []
        current_block: Optional[list[str]] = None
        for line in lines:
            if line.startswith("#") or line.startswith("-"):
                continue
            if line.startswith("$"):
                if current_block is not None:
                    calls_raw.append(current_block)
                current_block = []
            if current_block is not None:
                current_block.append(line)
        if current_block is not None:
            calls_raw.append(current_block)

        source_calls: dict[int, tuple[str, str, list[str]]] = {}
        for block in calls_raw:
            if not block:
                continue
            title = block[0]
            try:
                addr = int(title[1:5], 16)
                sym = title[7:13].strip()
                summary = title[15:].strip()
                details = [l[15:].strip() for l in block[1:] if l.strip()]
                source_calls[addr] = (sym, summary, details)
                if sym and addr >= 0xFF81:
                    all_symbols_for_addr[addr].append(sym)
            except (ValueError, IndexError):
                pass
        raw_sources[src_key] = source_calls

    # Second pass: determine canonical symbol for each address using majority vote
    for addr in sorted(set(all_symbols_for_addr.keys()) | set(KERNAL_JUMP_TARGETS.keys())):
        symbols_list = all_symbols_for_addr.get(addr, [])
        if symbols_list:
            ctr = collections.Counter(symbols_list)
            canonical = ctr.most_common(1)[0][0]
        else:
            canonical = f"KERNAL_{addr:04X}"

        entry = KernalEntry(addr, canonical)
        for src_key, calls in raw_sources.items():
            if addr in calls:
                sym, summary, details = calls[addr]
                entry.symbols_by_source[src_key] = sym
                entry.summaries_by_source[src_key] = summary
                entry.details_by_source[src_key] = details

        entries[addr] = entry

    return entries


def parse_disassembly_file(disasm_file: Path) -> dict[int, str]:
    """Parse disassembly file (e.g. c64disasm_cbm.txt) to extract routine and label symbols."""
    if not disasm_file.is_file():
        return {}

    with open(disasm_file, encoding="utf-8", errors="ignore") as f:
        lines = [line.rstrip() for line in f]

    disasm_labels: dict[int, str] = {}
    pending_label: Optional[str] = None

    for line in lines:
        if line.startswith("-") or line.startswith("#"):
            continue

        # Disassembly code (.,ADDR) or data (.:ADDR) line
        m_disasm = re.match(r"^\.[,:]\s*([0-9A-Fa-f]{4})", line)
        if m_disasm:
            try:
                addr = int(m_disasm.group(1), 16)
            except ValueError:
                pending_label = None
                continue

            src_part = line[32:] if len(line) > 32 else ""
            lbl = pending_label
            pending_label = None

            if not lbl and src_part and not src_part.startswith(" "):
                tokens = src_part.strip().split()
                if tokens and is_valid_label(tokens[0]):
                    lbl = tokens[0]

            if lbl and addr >= 0xE000:
                disasm_labels[addr] = lbl
            continue

        # Check for standalone label line before code
        trimmed = line.strip()
        if trimmed.startswith(";") or trimmed.startswith("."):
            pending_label = None
            continue

        if len(line) >= 32:
            src = line[32:]
            tokens = src.strip().split()
            if tokens and is_valid_label(tokens[0]):
                pending_label = tokens[0]
                continue

        pending_label = None

    return disasm_labels


def parse_lower_page_symbols(symbols_file: Path, prefix: str = "ZP_") -> dict[str, str]:
    """Parse zero-page and lower-page memory symbols from c64ref/src/c64mem/symbols.txt."""
    if not symbols_file.is_file():
        return {}

    lower_page: dict[str, str] = {}
    with open(symbols_file, encoding="utf-8", errors="ignore") as f:
        for line in f:
            line_str = line.strip()
            if not line_str.startswith("$"):
                continue
            parts = line_str.split()
            if len(parts) >= 2:
                addr_hex = parts[0][1:].upper().zfill(4)
                sym = parts[1].strip()
                if is_valid_label(sym):
                    lower_page[addr_hex] = f"{prefix}{sym}"
    return lower_page


def build_kernal_labels_and_comments(
    c64ref_dir: Path,
    mode: str = "api",
    comment_source: str = "fk",
    disasm_source: str = "cbm",
    prefix: str = "KERNAL_",
) -> Tuple[dict[str, str], dict[str, str]]:
    """Build the dictionary of unique labels and comments for KERNAL ROM."""
    kernal_dir = c64ref_dir / "src" / "kernal"
    disasm_filename = DISASM_FILES.get(disasm_source, "c64disasm_cbm.txt")
    disasm_file = c64ref_dir / "src" / "c64disasm" / disasm_filename

    api_entries = parse_kernal_api_dir(kernal_dir) if kernal_dir.is_dir() else {}
    cbm_labels = parse_disassembly_file(disasm_file) if (mode in ("disasm", "all", "full") and disasm_file.is_file()) else {}

    labels: dict[str, str] = {}
    comments: dict[str, str] = {}
    used_names: set[str] = set()

    # Helper to register unique label
    def add_label(addr_int: int, name: str) -> None:
        addr_hex = f"{addr_int:04X}"
        if addr_hex in labels:
            return
        unique_name = name
        # If label name collision occurs with a different address, disambiguate
        if unique_name in used_names:
            unique_name = f"{name}_{addr_hex}"
        labels[addr_hex] = unique_name
        used_names.add(unique_name)

    # 1. Add hardware and interrupt vectors (highest priority)
    for addr, vec_label in sorted(HARDWARE_VECTORS.items()):
        add_label(addr, vec_label)
        addr_hex = f"{addr:04X}"
        if addr in (0xFFFA, 0xFFFB):
            comments[addr_hex] = "NMI"
        elif addr in (0xFFFC, 0xFFFD):
            comments[addr_hex] = "Hardware Reset"
        elif addr in (0xFFFE, 0xFFFF):
            comments[addr_hex] = "IRQ"

    # 2. Add standard KERNAL Jump Table entries ($FF81 - $FFF3)
    for addr in sorted(api_entries.keys()):
        if addr < 0xFF81:
            continue
        addr_hex = f"{addr:04X}"
        entry = api_entries[addr]
        add_label(addr, f"{prefix}{entry.canonical_symbol}")

        summary = entry.get_summary(comment_source)
        if summary:
            comments[addr_hex] = f"${addr_hex} - {summary}"

    # 3. Add internal jump targets ($E500, $FDA3, etc.)
    for jmp_addr, target_addr in sorted(KERNAL_JUMP_TARGETS.items()):
        api_entry = api_entries.get(jmp_addr)
        sym = api_entry.canonical_symbol if api_entry else f"{jmp_addr:04X}"
        addr_hex = f"{target_addr:04X}"
        add_label(target_addr, f"{prefix}{sym}j")

        if api_entry:
            summary = api_entry.get_summary(comment_source)
            if summary:
                comments[addr_hex] = f"${addr_hex} (jmp) - {summary}"

    # 4. Add disassembly labels if mode is disasm, all, or full
    if mode in ("disasm", "all", "full"):
        for addr, sym in sorted(cbm_labels.items()):
            if not is_valid_label(sym):
                continue
            add_label(addr, f"{prefix}{sym}")

    return labels, comments


def generate_toml_snippet(labels: dict[str, str], comments: Optional[dict[str, str]] = None) -> str:
    """Format labels and optional comments as a TOML string."""
    out: list[str] = []
    out.append("[labels.KERNAL]")
    for addr in sorted(labels.keys()):
        out.append(f'"{addr}" = "{labels[addr]}"')
    out.append("")
    if comments:
        out.append("[comments]")
        for addr in sorted(comments.keys()):
            out.append(f'"{addr}" = "{comments[addr]}"')
        out.append("")
    return "\n".join(out)


def update_system_toml(
    target_toml: Path,
    new_kernal_labels: dict[str, str],
    new_comments: Optional[dict[str, str]] = None,
    new_lower_page: Optional[dict[str, str]] = None,
) -> None:
    """Update system-commodore_64.toml in place, preserving existing sections and formatting."""
    content = target_toml.read_text(encoding="utf-8")
    lines = content.splitlines()

    # Helper function to find start and end line index for a given section
    def find_section_range(section_name: str, lines_list: list[str]) -> Tuple[Optional[int], Optional[int]]:
        sec_start = None
        for i, line in enumerate(lines_list):
            if line.strip() == f"[{section_name}]":
                sec_start = i
                break
        if sec_start is None:
            return None, None
        sec_end = len(lines_list)
        for i in range(sec_start + 1, len(lines_list)):
            if lines_list[i].strip().startswith("[") and lines_list[i].strip().endswith("]"):
                sec_end = i
                break
        return sec_start, sec_end

    # 1. Update [labels.KERNAL] section
    kernal_start, kernal_end = find_section_range("labels.KERNAL", lines)
    kernal_block = ["[labels.KERNAL]"]
    for addr in sorted(new_kernal_labels.keys()):
        kernal_block.append(f'"{addr}" = "{new_kernal_labels[addr]}"')

    if kernal_start is not None and kernal_end is not None:
        lines = lines[:kernal_start] + kernal_block + [""] + lines[kernal_end:]
    else:
        lines.extend([""] + kernal_block + [""])

    # 2. Update [labels."Lower Page"] if requested
    if new_lower_page is not None:
        lp_start, lp_end = find_section_range('labels."Lower Page"', lines)
        lp_block = ['[labels."Lower Page"]']
        for addr in sorted(new_lower_page.keys()):
            lp_block.append(f'"{addr}" = "{new_lower_page[addr]}"')
        if lp_start is not None and lp_end is not None:
            lines = lines[:lp_start] + lp_block + [""] + lines[lp_end:]
        else:
            lines.extend([""] + lp_block + [""])

    # 3. Update [comments] section (only if new_comments is provided)
    if new_comments is not None:
        comments_start, comments_end = find_section_range("comments", lines)
        merged_comments: dict[str, str] = {}
        if comments_start is not None and comments_end is not None:
            for line in lines[comments_start + 1 : comments_end]:
                m = re.match(r'^"([0-9A-Fa-f]{4})"\s*=\s*"(.*)"$', line.strip())
                if m:
                    merged_comments[m.group(1).upper()] = m.group(2)

        merged_comments.update(new_comments)

        comments_block = ["[comments]"]
        for addr in sorted(merged_comments.keys()):
            comments_block.append(f'"{addr}" = "{merged_comments[addr]}"')

        if comments_start is not None and comments_end is not None:
            lines = lines[:comments_start] + comments_block + lines[comments_end:]
        else:
            lines.extend([""] + comments_block)

    # Normalize double blank lines
    normalized_lines: list[str] = []
    prev_blank = False
    for line in lines:
        is_blank = not line.strip()
        if is_blank and prev_blank:
            continue
        normalized_lines.append(line)
        prev_blank = is_blank

    target_toml.write_text("\n".join(normalized_lines) + "\n", encoding="utf-8")
    status_parts = [f"{len(new_kernal_labels)} KERNAL labels"]
    if new_comments is not None:
        status_parts.append(f"{len(merged_comments)} comments")
    if new_lower_page is not None:
        status_parts.append(f"{len(new_lower_page)} lower page symbols")
    print(f"Successfully updated {target_toml} with {', '.join(status_parts)}.")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Convert Commodore 64 KERNAL labels and comments from c64ref into TOML format for Regenerator 2000."
    )
    parser.add_argument(
        "--c64ref-dir",
        type=Path,
        default=Path(os.environ.get("C64REF_DIR", "/Users/ricardoq/src/c64/c64ref")),
        help="Path to c64ref directory (default: /Users/ricardoq/src/c64/c64ref)",
    )
    parser.add_argument(
        "--mode",
        choices=["api", "disasm", "all", "lower-page", "full"],
        default="api",
        help="Extraction mode: 'api' (KERNAL Jump Table $FF81-$FFF3 + jump targets), 'disasm' (Commodore source disassembly), 'all' (API + disasm), 'lower-page' (ZP symbols), 'full' (all KERNAL + Lower Page) (default: api)",
    )
    parser.add_argument(
        "--comment-source",
        choices=list(COMMENTARY_FILES.keys()),
        default="fk",
        help="Commentary source for routine descriptions (default: fk - Frank Kontros)",
    )
    parser.add_argument(
        "--disasm-source",
        choices=list(DISASM_FILES.keys()),
        default="cbm",
        help="Disassembly source file to use (default: cbm - Official Commodore KERNAL source)",
    )
    parser.add_argument(
        "--prefix",
        default="KERNAL_",
        help="Prefix for KERNAL labels (default: 'KERNAL_')",
    )
    parser.add_argument(
        "--zp-prefix",
        default="ZP_",
        help="Prefix for Zero Page / Lower Page labels (default: 'ZP_')",
    )
    parser.add_argument(
        "--no-comments",
        "--labels-only",
        action="store_true",
        dest="labels_only",
        help="Only extract/update [labels.KERNAL] without touching [comments] or other sections",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output TOML file path (default: stdout)",
    )
    parser.add_argument(
        "--update-toml",
        type=Path,
        default=None,
        help="Path to system-commodore_64.toml to update in place",
    )

    args = parser.parse_args()

    if not args.c64ref_dir.is_dir():
        sys.stderr.write(f"Error: c64ref directory not found at: {args.c64ref_dir}\n")
        return 1

    lower_page_symbols = None
    if args.mode in ("lower-page", "full"):
        symbols_file = args.c64ref_dir / "src" / "c64mem" / "symbols.txt"
        lower_page_symbols = parse_lower_page_symbols(symbols_file, prefix=args.zp_prefix)

    labels, comments = build_kernal_labels_and_comments(
        c64ref_dir=args.c64ref_dir,
        mode=args.mode,
        comment_source=args.comment_source,
        disasm_source=args.disasm_source,
        prefix=args.prefix,
    )

    comments_to_write = None if args.labels_only else comments

    if args.update_toml:
        if not args.update_toml.is_file():
            sys.stderr.write(f"Error: target TOML file not found: {args.update_toml}\n")
            return 1
        update_system_toml(
            target_toml=args.update_toml,
            new_kernal_labels=labels,
            new_comments=comments_to_write,
            new_lower_page=lower_page_symbols,
        )
    else:
        toml_str = generate_toml_snippet(labels, comments_to_write)
        if args.output:
            args.output.write_text(toml_str, encoding="utf-8")
            print(f"Wrote {len(labels)} labels to {args.output}")
        else:
            sys.stdout.write(toml_str)

    return 0


if __name__ == "__main__":
    sys.exit(main())
