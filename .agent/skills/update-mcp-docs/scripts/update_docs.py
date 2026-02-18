import requests
import json
import sys
import os
import time
import threading

BASE_URL = "http://localhost:3000/mcp"
DOCS_PATH = "docs/mcp.md"


class MCPClient:
    def __init__(self):
        self.session_id = None
        self.msg_id = 0
        self.responses = {}
        self.lock = threading.Lock()
        self.connected = threading.Event()
        self.sse_response = None

    def start(self):
        """Initialize MCP session and open SSE stream."""
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
            response = requests.post(
                BASE_URL,
                json=init_payload,
                headers={
                    "Accept": "application/json, text/event-stream",
                    "Content-Type": "application/json"
                },
                stream=True,
                timeout=10
            )
            response.raise_for_status()

            self.session_id = response.headers.get("mcp-session-id")
            if not self.session_id:
                raise Exception("No mcp-session-id in response headers")

            # Drain the init response stream
            for line in response.iter_lines():
                pass
            response.close()

            # Open a dedicated GET stream for SSE responses
            self.sse_response = requests.get(
                BASE_URL,
                headers={
                    "Accept": "text/event-stream",
                    "mcp-session-id": self.session_id
                },
                stream=True,
                timeout=None
            )
            self.sse_response.raise_for_status()

            # Start SSE listener thread
            t = threading.Thread(target=self._listen_sse, daemon=True)
            t.start()

            # Wait for SSE stream to be ready
            if not self.connected.wait(timeout=5):
                raise Exception("Timeout waiting for SSE stream")

            # Send initialized notification (no id, it's a notification)
            notif = {
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {}
            }
            requests.post(
                BASE_URL,
                json=notif,
                headers={
                    "Accept": "application/json, text/event-stream",
                    "Content-Type": "application/json",
                    "mcp-session-id": self.session_id
                },
                timeout=5
            )

        except Exception as e:
            print(f"Connection failed: {e}")
            return False
        return True

    def _listen_sse(self):
        self.connected.set()
        try:
            for line in self.sse_response.iter_lines():
                if line:
                    decoded = line.decode("utf-8")
                    if decoded.startswith("data:"):
                        data = decoded[5:].strip()
                        if data:
                            try:
                                msg = json.loads(data)
                                if "id" in msg:
                                    with self.lock:
                                        self.responses[msg["id"]] = msg
                            except json.JSONDecodeError:
                                pass
        except Exception:
            pass

    def _wait_for_response(self, msg_id, timeout=10):
        start = time.time()
        while time.time() - start < timeout:
            with self.lock:
                if msg_id in self.responses:
                    return self.responses.pop(msg_id)
            time.sleep(0.05)
        return None

    def rpc(self, method, params={}):
        """Send a JSON-RPC request and wait for the response via SSE."""
        self.msg_id += 1
        current_id = self.msg_id

        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": current_id
        }

        headers = {
            "Accept": "application/json, text/event-stream",
            "Content-Type": "application/json",
            "mcp-session-id": self.session_id
        }

        try:
            resp = requests.post(BASE_URL, json=payload, headers=headers, stream=True, timeout=10)
            # Drain any inline response (202 Accepted or SSE in POST body)
            for line in resp.iter_lines():
                if line:
                    decoded = line.decode("utf-8")
                    if decoded.startswith("data:"):
                        data = decoded[5:].strip()
                        if data:
                            try:
                                msg = json.loads(data)
                                if msg.get("id") == current_id:
                                    resp.close()
                                    return msg
                                elif "id" in msg:
                                    with self.lock:
                                        self.responses[msg["id"]] = msg
                            except json.JSONDecodeError:
                                pass
            resp.close()
        except Exception as e:
            print(f"RPC send failed: {e}")

        # Fall back to waiting on the SSE stream
        return self._wait_for_response(current_id)

    def close(self):
        if self.sse_response:
            try:
                self.sse_response.close()
            except Exception:
                pass


def fetch_tools():
    client = MCPClient()
    if not client.start():
        return []

    print("Fetching tools list...")
    res = client.rpc("tools/list")
    client.close()

    if res and "result" in res and "tools" in res["result"]:
        return res["result"]["tools"]
    else:
        print(f"Error fetching tools: {res}")
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
                if isinstance(p_type, list):
                    p_type = " \\| ".join(p_type)
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
