import requests
import json
import sys
import os

BASE_URL = "http://localhost:3000/mcp"
DOCS_PATH = "docs/mcp.md"

def fetch_tools():
    print(f"Connecting to {BASE_URL}...")
    # Initialize
    init_payload = {
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "docs_updater", "version": "1.0"}
        },
        "id": 1
    }

    try:
        resp = requests.post(BASE_URL, json=init_payload, timeout=5)
        resp.raise_for_status()
        print("Initialized.")

        # Get Session ID
        session_id = resp.headers.get("mcp-session-id")
        headers = {"mcp-session-id": session_id} if session_id else {}

        # List Tools
        tool_payload = {
            "jsonrpc": "2.0",
            "method": "tools/list",
            "params": {},
            "id": 2
        }
        resp = requests.post(BASE_URL, json=tool_payload, headers=headers, timeout=5)
        data = resp.json()

        if "result" in data and "tools" in data["result"]:
            return data["result"]["tools"]
        else:
            print("Error: unexpected response format for tools/list")
            return []

    except Exception as e:
        print(f"Error fetching tools: {e}")
        return []

def generate_markdown(tools):
    md = []
    md.append("## Available Tools")
    md.append("")
    md.append(f"The server currently exposes **{len(tools)}** tools.")
    md.append("")

    # Sort tools by name
    tools.sort(key=lambda x: x["name"])

    for tool in tools:
        name = tool["name"]
        desc = tool.get("description", "No description provided.")
        md.append(f"### `{name}`")
        md.append("")
        md.append(f"{desc}")
        md.append("")

        schema = tool.get("inputSchema", {})
        props = schema.get("properties", {})
        required = schema.get("required", [])

        if props:
            md.append("**Arguments:**")
            md.append("")
            md.append("| Name | Type | Description | Required |")
            md.append("| :--- | :--- | :--- | :---: |")

            for prop_name, prop_data in props.items():
                p_type = prop_data.get("type", "any")
                p_desc = prop_data.get("description", "-")
                is_req = "Yes" if prop_name in required else "No"
                md.append(f"| `{prop_name}` | `{p_type}` | {p_desc} | {is_req} |")
            md.append("")
        else:
            md.append("_No arguments._")
            md.append("")

    return "\n".join(md)

def update_file(new_content):
    if not os.path.exists(DOCS_PATH):
        print(f"Error: {DOCS_PATH} not found.")
        return

    with open(DOCS_PATH, "r") as f:
        content = f.read()

    # Find the marker
    marker = "## Available Tools"
    if marker in content:
        print("Found existing 'Available Tools' section. Replacing...")
        pre_content = content.split(marker)[0]
        final_content = pre_content + new_content
    else:
        print("Appending 'Available Tools' section...")
        final_content = content + "\n\n" + new_content

    with open(DOCS_PATH, "w") as f:
        f.write(final_content)
    print(f"Successfully updated {DOCS_PATH}")

if __name__ == "__main__":
    tools = fetch_tools()
    if tools:
        md = generate_markdown(tools)
        update_file(md)
    else:
        print("No tools found or connection failed.")
        sys.exit(1)
