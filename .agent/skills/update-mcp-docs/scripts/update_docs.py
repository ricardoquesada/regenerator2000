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


def generate_markdown(tools: list[dict]) -> str:
    """Render the '## Available Tools' Markdown section with a trailing newline."""
    lines = [
        SECTION_HEADER,
        "",
        f"The server exposes **{len(tools)}** tools.",
        "",
    ]

    for tool in sorted(tools, key=lambda item: item["name"]):
        name = tool["name"]
        desc = tool.get("description", "No description provided.")
        lines.extend([f"### `{name}`", "", desc, ""])

        schema = tool.get("inputSchema", {})
        props = schema.get("properties", {})
        required = set(schema.get("required", []))

        if props:
            lines.extend(
                [
                    "**Arguments:**",
                    "",
                    "| Name | Type | Description | Required |",
                    "| :--- | :--- | :--- | :---: |",
                ]
            )
            for prop_name, prop_data in sorted(props.items()):
                p_type = prop_data.get("type", "any")
                if isinstance(p_type, list):
                    p_type = " \\| ".join(p_type)
                p_desc = escape_table_cell(prop_data.get("description", "-"))
                is_req = "Yes" if prop_name in required else "No"
                lines.append(f"| `{prop_name}` | `{p_type}` | {p_desc} | {is_req} |")
            lines.append("")
        else:
            lines.extend(["_No arguments._", ""])

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
