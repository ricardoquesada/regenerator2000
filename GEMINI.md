# Gemini Model Configuration & Efficiency Plan

## Model Routing Rules
- **Planning & Architecture:** Use `gemini-3-pro` with `thinking_level: high`.
  - Use this for: Designing the opcode trait system, mapping C64 memory mirrors, or solving TUI state management.
- **Implementation & Refactoring:** Use `gemini-3-flash` (Gemini Fast).
  - Use this for: Writing repetitive opcode match arms, boilerplate Ratatui widgets, and unit tests.

## Token Saving Strategies
- **Context Pinning:** Only provide the `src/disasm/` directory when working on logic; do not include the TUI code unless fixing layout issues.
- **TOON Encoding:** For large 6502 opcode tables or lookup JSONs, use TOON encoding to reduce syntactic overhead.
- **Incremental Diffing:** Use the `edit_file` skill instead of re-generating whole files to keep the output token count minimal.

## Quality Control
- For every disassembly logic change, the agent must verify the byte-to-mnemonic mapping against known 6502 standards.
- Prefer "Dry Run" plans for complex Rust lifetime issues before writing code.
