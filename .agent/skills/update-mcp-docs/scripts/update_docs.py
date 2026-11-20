#!/usr/bin/env python3
"""Syncs the '## Available Tools' section in docs/mcp.md via --mcp-server-stdio."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT_DIR = Path(__file__).resolve().parents[4]
DEFAULT_DOCS_PATH = ROOT_DIR / "docs" / "mcp.md"
SECTION_HEADER = "## Available Tools"
SECTION_RE = re.compile(r"^## Available Tools\b.*?(?=^## |\Z)", re.MULTILINE | re.DOTALL)

TOOL_CATEGORIES: list[tuple[str, tuple[str, ...]]] = [
    (
        "Inspection & Reading",
        (
            "r2000_get_binary_info",
            "r2000_get_blocks",
            "r2000_read_region",
            "r2000_read_selected",
            "r2000_get_address_details",
            "r2000_get_symbols",
            "r2000_get_comments",
            "r2000_get_cross_references",
        ),
    ),
    (
        "Disassembly & Editing",
        (
            "r2000_disassemble",
            "r2000_set_data_type",
            "r2000_set_label_name",
            "r2000_set_comment",
            "r2000_set_immediate_format",
            "r2000_set_symbolic_offset",
            "r2000_add_scope",
            "r2000_toggle_splitter",
        ),
    ),
    (
        "Enums",
        (
            "r2000_create_project_enum",
            "r2000_update_project_enum",
            "r2000_delete_project_enum",
            "r2000_apply_enum_usage",
        ),
    ),
    (
        "Navigation & Search",
        (
            "r2000_get_disassembly_cursor",
            "r2000_jump_to_address",
            "r2000_search_disassembly",
            "r2000_search_memory",
        ),
    ),
    (
        "Session, Batching & Unpacking",
        (
            "r2000_batch_execute",
            "r2000_unpack_binary",
            "r2000_save_project",
            "r2000_undo",
            "r2000_redo",
        ),
    ),
]


def fetch_tools() -> list[dict]:
    """Query tools/list from regenerator2000 in headless stdio MCP mode."""
    request = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}
    )
    proc = subprocess.run(
        ["cargo", "run", "--quiet", "--", "--mcp-server-stdio"],
        cwd=ROOT_DIR,
        input=f"{request}\n",
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if proc.returncode != 0:
        sys.stderr.write(
            f"MCP stdio server failed (exit {proc.returncode}):\n{proc.stderr}\n"
        )
        sys.exit(1)

    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        msg = json.loads(line)
        if msg.get("id") == 1:
            tools = msg.get("result", {}).get("tools")
            if isinstance(tools, list):
                return tools
            sys.stderr.write(f"Unexpected tools/list payload: {msg}\n")
            sys.exit(1)

    sys.stderr.write("No tools/list response received from --mcp-server-stdio.\n")
    sys.exit(1)


def escape_table_cell(text: str) -> str:
    """Escape unescaped pipe characters and newlines inside a Markdown table cell."""
    collapsed = " ".join(text.splitlines())
    return re.sub(r"(?<!\\)\|", r"\\|", collapsed)


def first_sentence(text: str) -> str:
    """Extract the concise first sentence of a tool description for summaries."""
    collapsed = " ".join(text.split())
    match = re.match(r"(.+?\.)(?:\s|$)", collapsed)
    return match.group(1) if match else collapsed


def group_tools(tools: list[dict]) -> list[tuple[str, list[dict]]]:
    """Group tools into ordered semantic categories with an 'Other Tools' fallback."""
    by_name = {tool["name"]: tool for tool in tools}
    assigned: set[str] = set()
    grouped: list[tuple[str, list[dict]]] = []

    for category, names in TOOL_CATEGORIES:
        items = [by_name[name] for name in names if name in by_name]
        if items:
            grouped.append((category, items))
            assigned.update(name for name in names if name in by_name)

    remaining = [
        tool for name, tool in sorted(by_name.items()) if name not in assigned
    ]
    if remaining:
        grouped.append(("Other Tools", remaining))

    return grouped


def generate_markdown(tools: list[dict]) -> str:
    """Render the '## Available Tools' Markdown section with grouped collapsible drawers."""
    grouped = group_tools(tools)
    lines = [
        SECTION_HEADER,
        "",
        (
            f"The server exposes **{len(tools)}** tools across **{len(grouped)}**"
            " categories. Expand any tool below to inspect its full description"
            " and argument schema."
        ),
        "",
    ]

    for category, items in grouped:
        lines.extend(
            [
                f"### {category}",
                "",
                "| Tool | Summary |",
                "| :--- | :--- |",
            ]
        )
        for tool in items:
            name = tool["name"]
            desc = tool.get("description", "No description provided.")
            summary = escape_table_cell(first_sentence(desc))
            lines.append(f"| [`{name}`](#{name}) | {summary} |")
        lines.append("")

        for tool in items:
            name = tool["name"]
            desc = " ".join(
                tool.get("description", "No description provided.").split()
            )
            summary = first_sentence(desc)
            lines.extend(
                [
                    f"/// details | `{name}` — {summary}",
                    "    type: abstract",
                    f"    attrs: {{id: {name}}}",
                    "",
                    desc,
                    "",
                ]
            )

            schema = tool.get("inputSchema", {})
            props = schema.get("properties", {})
            required = set(schema.get("required", []))

            if props:
                lines.extend(
                    [
                        "| Argument | Type | Required | Description |",
                        "| :--- | :--- | :---: | :--- |",
                    ]
                )
                for prop_name, prop_data in sorted(props.items()):
                    p_type = prop_data.get("type", "any")
                    if isinstance(p_type, list):
                        p_type = " \\| ".join(p_type)
                    p_desc = escape_table_cell(prop_data.get("description", "-"))
                    is_req = "Yes" if prop_name in required else "No"
                    lines.append(
                        f"| `{prop_name}` | `{p_type}` | {is_req} | {p_desc} |"
                    )
            else:
                lines.append("_No arguments._")

            lines.extend(["///", ""])

    return "\n".join(lines).rstrip() + "\n"


def update_file(docs_path: Path, new_section: str) -> None:
    """Replace or append '## Available Tools' in docs_path without dropping later sections."""
    if not docs_path.is_file():
        sys.stderr.write(f"Error: {docs_path} not found.\n")
        sys.exit(1)

    content = docs_path.read_text(encoding="utf-8")
    if SECTION_RE.search(content):
        updated = SECTION_RE.sub(lambda _: new_section, content, count=1)
    else:
        updated = content.rstrip() + "\n\n" + new_section

    if not updated.endswith("\n"):
        updated += "\n"

    docs_path.write_text(updated, encoding="utf-8")
    print(f"Updated {docs_path} ({new_section.splitlines()[2]})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_DOCS_PATH,
        help="Target Markdown file to update (defaults to docs/mcp.md).",
    )
    args = parser.parse_args()

    tools = fetch_tools()
    if not tools:
        sys.stderr.write("Error: server returned zero tools.\n")
        sys.exit(1)

    update_file(args.output.resolve(), generate_markdown(tools))


if __name__ == "__main__":
    main()
