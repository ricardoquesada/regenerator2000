import requests
import json
import sys
import time
import threading

BASE_URL = "http://localhost:3000/mcp"


class MCPClient:
    def __init__(self):
        self.session_id = None
        self.msg_id = 0
        self.responses = {}
        self.lock = threading.Lock()
        self.connected = threading.Event()
        self.read_thread = None

    def start(self):
        print(f"Connecting to MCP Server at {BASE_URL}...")

        # 1. Initialize via POST (Capture Session ID)
        init_payload = {
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "verify_mcp",
                    "version": "1.0"
                }
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
                print("Error: No mcp-session-id in response headers")
                sys.exit(1)

            print(f"Connected. Session ID: {self.session_id}")

            init_response = None
            for line in response.iter_lines():
                if line:
                    decoded_line = line.decode('utf-8')
                    if decoded_line.startswith("data:"):
                        data = decoded_line[5:].strip()
                        if data and data != '':
                            try:
                                msg = json.loads(data)
                                if msg.get("id") == 1:
                                    init_response = msg
                                    break
                            except json.JSONDecodeError:
                                continue

            response.close()

            if not init_response or "result" not in init_response:
                print("Failed to get initialization response")
                sys.exit(1)

            print("Initialized successfully.")

            get_response = requests.get(
                BASE_URL,
                headers={
                    "Accept": "text/event-stream",
                    "mcp-session-id": self.session_id
                },
                stream=True,
                timeout=None
            )
            get_response.raise_for_status()

            self.read_thread = threading.Thread(target=self._listen_sse, args=(get_response,), daemon=True)
            self.read_thread.start()

            if not self.connected.wait(timeout=5):
                print("Timeout waiting for GET stream connection")
                sys.exit(1)

            self.rpc("notifications/initialized", {})

        except Exception as e:
            print(f"Connection failed: {e}")
            sys.exit(1)


    def _listen_sse(self, initial_response):
        response = initial_response

        while True:
            self.connected.set()
            try:
                for line in response.iter_lines():
                    if line:
                        decoded_line = line.decode('utf-8')
                        if decoded_line.startswith("event:"):
                            continue
                        if decoded_line.startswith("data:"):
                            data = decoded_line[5:].strip()
                            if not data:
                                continue
                            try:
                                msg = json.loads(data)
                                self._handle_message(msg)
                            except json.JSONDecodeError:
                                print(f"Failed to decode JSON: {data}")
            except Exception as e:
                print(f"SSE stream error: {e}")
            finally:
                self.connected.clear()
                if response:
                    response.close()

            print("SSE stream ended. Reconnecting...")
            time.sleep(1)

            if not self.session_id:
                print("No session ID, cannot reconnect.")
                break

            try:
                headers = {
                    "Accept": "text/event-stream",
                    "mcp-session-id": self.session_id
                }
                print(f"Reconnecting to {BASE_URL} with session {self.session_id}")
                response = requests.get(
                    BASE_URL,
                    headers=headers,
                    stream=True,
                    timeout=None
                )
                response.raise_for_status()
                print("Reconnected.")
                self.connected.set()
            except Exception as e:
                print(f"Reconnection failed: {e}")
                time.sleep(2)

    def _handle_message(self, msg):
        if "id" in msg:
            with self.lock:
                self.responses[msg["id"]] = msg
        elif "method" in msg:
            pass

    def _wait_for_response(self, msg_id, timeout=5):
        start_time = time.time()
        while time.time() - start_time < timeout:
            with self.lock:
                if msg_id in self.responses:
                    return self.responses.pop(msg_id)
            time.sleep(0.1)
        return None

    def rpc(self, method, params={}):
        if not self.connected.is_set():
            print("Waiting for connection...")
            if not self.connected.wait(timeout=10):
                print("Timeout waiting for connection")
                return None

        is_notification = method.startswith("notifications/")

        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }

        current_id = None
        if not is_notification:
            self.msg_id += 1
            current_id = self.msg_id
            payload["id"] = current_id

        headers = {
            "Accept": "application/json, text/event-stream",
            "Content-Type": "application/json",
            "mcp-session-id": self.session_id
        }

        try:
            response = requests.post(BASE_URL, json=payload, headers=headers, stream=True, timeout=10)

            if response.status_code == 200:
                if "text/event-stream" in response.headers.get("Content-Type", ""):
                    for line in response.iter_lines():
                        if line:
                            decoded_line = line.decode('utf-8')
                            if decoded_line.startswith("data:"):
                                data = decoded_line[5:].strip()
                                if data:
                                    try:
                                        msg = json.loads(data)
                                        if msg.get("id") == current_id:
                                            response.close()
                                            return msg
                                        else:
                                            self._handle_message(msg)
                                    except json.JSONDecodeError:
                                        continue
                else:
                    try:
                        json_resp = response.json()
                        if "result" in json_resp or "error" in json_resp:
                            return json_resp
                    except:
                        pass
            elif response.status_code == 202:
                pass
            else:
                print(f"Request failed with status {response.status_code}: {response.text}")
                response.raise_for_status()

            if is_notification:
                response.close()
                return None

            res = self._wait_for_response(current_id)
            if res:
                response.close()
                return res

            print(f"Timeout waiting for response to {method}")
            response.close()
            return None

        except Exception as e:
            print(f"Request failed: {e}")
            return None


# ---------------------------------------------------------------------------
# Test helpers
# ---------------------------------------------------------------------------

EXPECTED_TOOLS = {
    "r2000_set_label_name",
    "r2000_set_comment",
    "r2000_set_data_type",
    "r2000_toggle_splitter",
    "r2000_undo",
    "r2000_redo",
    "r2000_read_region",
    "r2000_read_selected",
    "r2000_get_binary_info",
    "r2000_get_analyzed_blocks",
    "r2000_get_address_details",
    "r2000_get_disassembly_cursor",
    "r2000_jump_to_address",
    "r2000_search_memory",
    "r2000_search_disassembly",
    "r2000_get_cross_references",
    "r2000_set_operand_format",
    "r2000_get_symbols",
    "r2000_get_comments",
    "r2000_save_project",
    "r2000_batch_execute",
    "r2000_add_scope",
}

REMOVED_TOOLS = {
    "r2000_convert_region_to_code",
    "r2000_convert_region_to_bytes",
    "r2000_convert_region_to_words",
    "r2000_convert_region_to_address",
    "r2000_convert_region_to_petscii",
    "r2000_convert_region_to_screencode",
    "r2000_convert_region_to_lo_hi_address",
    "r2000_convert_region_to_hi_lo_address",
    "r2000_convert_region_to_lo_hi_word",
    "r2000_convert_region_to_hi_lo_word",
    "r2000_convert_region_to_external_file",
    "r2000_convert_region_to_undefined",
    "r2000_set_side_comment",
    "r2000_set_line_comment",
    "r2000_read_disasm_region",
    "r2000_read_hexdump_region",
    "r2000_read_selected_disasm",
    "r2000_read_selected_hexdump",
    "r2000_get_symbol_table",
    "r2000_get_all_comments",
}


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

def test_list_tools(client):
    print("\nTesting tools/list...")
    res = client.rpc("tools/list")
    if not (res and "result" in res):
        print(f"FAIL: {res}")
        return

    tools = res["result"]["tools"]
    names = set(t["name"] for t in tools)
    print(f"Found {len(tools)} tools: {sorted(names)}")

    missing = EXPECTED_TOOLS - names
    unexpected = REMOVED_TOOLS & names

    if missing:
        print(f"FAIL: Missing expected tools: {missing}")
    if unexpected:
        print(f"FAIL: Old/removed tools still present: {unexpected}")
    if not missing and not unexpected:
        print(f"PASS: All {len(EXPECTED_TOOLS)} expected tools present, no removed tools found.")


def test_set_label(client):
    print("\nTesting r2000_set_label_name...")
    res = client.rpc("tools/call", {
        "name": "r2000_set_label_name",
        "arguments": {
            "address": 4096,
            "name": "TEST_LABEL"
        }
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        print(f"Tool output: {text}")
        if "Label set at" in text:
            print("PASS")
        else:
            print("FAIL: Tool response missing confirmation text")
    elif res and "error" in res:
        print(f"Error (may be OOB): {res['error']}")
    else:
        print(f"FAIL: {res}")


def test_set_comment(client):
    print("\nTesting r2000_set_comment (line)...")
    res = client.rpc("tools/call", {
        "name": "r2000_set_comment",
        "arguments": {
            "address": 0x1000,
            "comment": "added by MCP test",
            "type": "line"
        }
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        print(f"Tool output: {text}")
        if "Comment set at" in text:
            print("PASS (line)")
        else:
            print("FAIL: Missing confirmation")
    else:
        print(f"FAIL: {res}")

    print("\nTesting r2000_set_comment (side)...")
    res = client.rpc("tools/call", {
        "name": "r2000_set_comment",
        "arguments": {
            "address": 0x1000,
            "comment": "side comment by MCP test",
            "type": "side"
        }
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        if "Comment set at" in text:
            print("PASS (side)")
        else:
            print("FAIL: Missing confirmation")
    else:
        print(f"FAIL: {res}")


def test_set_data_type(client):
    print("\nTesting r2000_set_data_type...")

    cases = [
        ("code",          0x1000, 0x100F),
        ("byte",          0x1010, 0x101F),
        ("word",          0x1020, 0x102F),
        ("address",       0x1030, 0x1033),
        ("petscii",       0x1040, 0x104F),
        ("screencode",    0x1050, 0x105F),
        ("lo_hi_address", 0x1060, 0x1067),
        ("hi_lo_address", 0x1068, 0x106F),
        ("lo_hi_word",    0x1070, 0x1077),
        ("hi_lo_word",    0x1078, 0x107F),
        ("external_file", 0x1080, 0x10FF),
        ("undefined",     0x1000, 0x10FF),  # reset everything at the end
    ]

    all_pass = True
    for data_type, start, end in cases:
        res = client.rpc("tools/call", {
            "name": "r2000_set_data_type",
            "arguments": {
                "start_address": start,
                "end_address": end,
                "data_type": data_type
            }
        })
        if res and "result" in res:
            content = res["result"].get("content", [])
            text = content[0].get("text", "") if content else ""
            if "converted to" in text.lower() or "Region" in text:
                print(f"  PASS: data_type='{data_type}'")
            else:
                print(f"  FAIL: data_type='{data_type}' — unexpected response: {text}")
                all_pass = False
        elif res and "error" in res:
            print(f"  FAIL: data_type='{data_type}' — error: {res['error']['message']}")
            all_pass = False
        else:
            print(f"  FAIL: data_type='{data_type}' — no response")
            all_pass = False

    if all_pass:
        print("PASS: r2000_set_data_type — all data_type values work")

    # Test unknown data_type returns an error
    print("  Testing unknown data_type returns error...")
    res = client.rpc("tools/call", {
        "name": "r2000_set_data_type",
        "arguments": {
            "start_address": 0x1000,
            "end_address": 0x1001,
            "data_type": "invalid_type_xyz"
        }
    })
    # r2000_set_data_type is handled inside handle_tool_call_internal which catches the match,
    # so the error comes back as a tool-level error in the result or McpError.
    # The server wraps it as result=None, error=McpError.
    if res and "error" in res:
        print("  PASS: unknown data_type correctly returned an error")
    elif res and "result" in res:
        # The error might be embedded inside the content (some MCP servers do this)
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        if "Unknown data_type" in text or "unknown" in text.lower():
            print("  PASS: unknown data_type returned error in content")
        else:
            print(f"  FAIL: unknown data_type did not return expected error: {res}")
    else:
        print(f"  FAIL: no response for unknown data_type: {res}")


def test_read_region(client):
    print("\nTesting r2000_read_region (disasm)...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_region",
        "arguments": {
            "start_address": 4096,
            "end_address": 4112,
            "view": "disasm"
        }
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        if text:
            print(f"PASS (disasm): snippet: {text[:60]!r}...")
        else:
            print("FAIL: empty content")
    else:
        print(f"FAIL: {res}")

    print("\nTesting r2000_read_region (hexdump)...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_region",
        "arguments": {
            "start_address": 4096,
            "end_address": 4112,
            "view": "hexdump"
        }
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        if text:
            print(f"PASS (hexdump): snippet: {text[:60]!r}...")
        else:
            print("FAIL: empty content")
    else:
        print(f"FAIL: {res}")

    print("\nTesting r2000_read_region (default view = disasm)...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_region",
        "arguments": {
            "start_address": 4096,
            "end_address": 4097
        }
    })
    if res and "result" in res:
        print("PASS (default view)")
    else:
        print(f"FAIL: {res}")


def test_read_selected(client):
    print("\nTesting r2000_read_selected (disasm)...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_selected",
        "arguments": {"view": "disasm"}
    })
    if res and "result" in res:
        print("PASS (disasm)")
        content = res["result"].get("content", [])
        if content:
            print(f"  Snippet: {content[0].get('text','')[:60]!r}")
    else:
        print(f"FAIL: {res}")

    print("\nTesting r2000_read_selected (hexdump)...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_selected",
        "arguments": {"view": "hexdump"}
    })
    if res and "result" in res:
        print("PASS (hexdump)")
    else:
        print(f"FAIL: {res}")

    print("\nTesting r2000_read_selected (default view)...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_selected",
        "arguments": {}
    })
    if res and "result" in res:
        print("PASS (default view)")
    else:
        print(f"FAIL: {res}")


def test_get_binary_info(client):
    print("\nTesting r2000_get_binary_info...")
    res = client.rpc("tools/call", {
        "name": "r2000_get_binary_info",
        "arguments": {}
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        try:
            data = json.loads(text)
            if "origin" in data and "size" in data and "platform" in data:
                print(f"PASS: origin={data['origin']}, size={data['size']}, platform={data['platform']!r}")
            else:
                print(f"FAIL: Missing fields: {list(data.keys())}")
        except json.JSONDecodeError:
            print(f"FAIL: Could not decode JSON: {text}")
    else:
        print(f"FAIL: {res}")


def test_misc_tools(client):
    print("\nTesting miscellaneous tools...")

    # Search Memory
    print("- r2000_search_memory")
    res = client.rpc("tools/call", {
        "name": "r2000_search_memory",
        "arguments": {"query": "A9 00"}
    })
    print("  PASS" if res and "result" in res else f"  FAIL: {res}")

    # Get Cross References
    print("- r2000_get_cross_references")
    res = client.rpc("tools/call", {
        "name": "r2000_get_cross_references",
        "arguments": {"address": 0x1000}
    })
    print("  PASS" if res and "result" in res else f"  FAIL: {res}")

    # Get Symbols
    print("- r2000_get_symbols")
    res = client.rpc("tools/call", {
        "name": "r2000_get_symbols",
        "arguments": {}
    })
    print("  PASS" if res and "result" in res else f"  FAIL: {res}")

    # Get Comments — verify structure
    print("- r2000_get_comments")
    res = client.rpc("tools/call", {
        "name": "r2000_get_comments",
        "arguments": {}
    })
    if res and "result" in res:
        try:
            content_text = res["result"]["content"][0]["text"]
            comments = json.loads(content_text)
            if isinstance(comments, list):
                print(f"  PASS: list of {len(comments)} comment(s)")
                if comments:
                    c = comments[0]
                    if "address" in c and "type" in c and "comment" in c:
                        print(f"  PASS: structure OK — {c}")
                    else:
                        print(f"  FAIL: bad structure: {c}")
            else:
                print("  FAIL: not a list")
        except Exception as e:
            print(f"  FAIL: {e}")
    else:
        print(f"  FAIL: {res}")

    # Set Operand Format
    print("- r2000_set_operand_format")
    res = client.rpc("tools/call", {
        "name": "r2000_set_operand_format",
        "arguments": {"address": 0x1000, "format": "binary"}
    })
    print("  PASS" if res and "result" in res else f"  FAIL: {res}")

    # Save Project — expect error (no project loaded)
    print("- r2000_save_project (expect error)")
    res = client.rpc("tools/call", {
        "name": "r2000_save_project",
        "arguments": {}
    })
    if res and "error" in res:
        print(f"  PASS: expected error — {res['error']['message']}")
    elif res and "result" in res:
        print("  PASS: saved (unexpected but valid)")
    else:
        print(f"  FAIL: {res}")


def test_search_disassembly(client):
    print("\nTesting r2000_search_disassembly...")

    def call(arguments):
        return client.rpc("tools/call", {
            "name": "r2000_search_disassembly",
            "arguments": arguments,
        })

    def get_results(res):
        """Parse the JSON result list from an MCP response."""
        try:
            text = res["result"]["content"][0]["text"]
            return json.loads(text)
        except Exception:
            return None

    # Plain-text search — should return a list (possibly empty on a blank project)
    print("- plain-text search (query='NOP')")
    res = call({"query": "NOP"})
    if res and "result" in res:
        results = get_results(res)
        if isinstance(results, list):
            print(f"  PASS: returned {len(results)} result(s)")
            if results:
                r = results[0]
                required_keys = {"address", "address_decimal", "label", "mnemonic", "operand", "comment"}
                if required_keys.issubset(r.keys()):
                    print(f"  PASS: result structure OK — {r['address']} {r['mnemonic']} {r['operand']}")
                else:
                    print(f"  FAIL: missing keys in result: {set(r.keys())}")
        else:
            print(f"  FAIL: expected list, got: {results}")
    else:
        print(f"  FAIL: {res}")

    # Regex search — valid pattern
    print("- regex search (query='NOP|BRK', use_regex=True)")
    res = call({"query": "NOP|BRK", "use_regex": True})
    if res and "result" in res:
        results = get_results(res)
        if isinstance(results, list):
            print(f"  PASS: regex search returned {len(results)} result(s)")
        else:
            print(f"  FAIL: expected list, got: {results}")
    else:
        print(f"  FAIL: {res}")

    # Regex search — invalid pattern must return an error
    print("- regex search with invalid pattern (expect error)")
    res = call({"query": "[unclosed", "use_regex": True})
    if res and "error" in res:
        print(f"  PASS: invalid regex correctly returned error — {res['error']['message'][:60]!r}")
    elif res and "result" in res:
        # Some servers surface errors inside content
        text = res["result"].get("content", [{}])[0].get("text", "")
        if "invalid regex" in text.lower() or "regex" in text.lower():
            print(f"  PASS (error in content): {text[:60]!r}")
        else:
            print(f"  FAIL: expected error for invalid regex, got: {text[:60]!r}")
    else:
        print(f"  FAIL: no response")

    # Filter flags — search only labels
    print("- filter: search_labels=True, search_comments=False, search_instructions=False")
    res = call({"query": "NOP", "search_labels": True, "search_comments": False, "search_instructions": False})
    if res and "result" in res:
        print("  PASS: filter flags accepted")
    else:
        print(f"  FAIL: {res}")

    # max_results cap
    print("- max_results=1")
    res = call({"query": "NOP", "max_results": 1})
    if res and "result" in res:
        results = get_results(res)
        if isinstance(results, list) and len(results) <= 1:
            print(f"  PASS: max_results respected ({len(results)} result(s))")
        else:
            print(f"  FAIL: got {len(results) if isinstance(results, list) else '?'} results with max_results=1")
    else:
        print(f"  FAIL: {res}")


def test_list_resources(client):
    print("\nTesting resources/list...")
    res = client.rpc("resources/list")
    if res and "result" in res:
        resources = res["result"]["resources"]
        uris = [r["uri"] for r in resources]
        print(f"Found {len(resources)} resources: {uris}")
        if "disasm://main" in uris and "binary://main" in uris:
            print("PASS")
        else:
            print("FAIL: Missing expected resources")
    else:
        print(f"FAIL: {res}")


def test_get_disassembly_cursor(client):
    print("\nTesting r2000_get_disassembly_cursor...")
    res = client.rpc("tools/call", {
        "name": "r2000_get_disassembly_cursor",
        "arguments": {}
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        print(f"PASS: cursor at {text}")
    elif res and "error" in res:
        print(f"PASS (valid error — empty project): {res['error']['message']}")
    else:
        print(f"FAIL: {res}")


def test_jump_to_address(client):
    print("\nTesting r2000_jump_to_address...")

    print("- Getting current cursor...")
    res1 = client.rpc("tools/call", {
        "name": "r2000_get_disassembly_cursor",
        "arguments": {}
    })
    if res1 and "result" in res1:
        text = res1["result"].get("content", [{}])[0].get("text", "?")
        print(f"  Current: {text}")

    print("- Jumping to $1000 (4096)...")
    res2 = client.rpc("tools/call", {
        "name": "r2000_jump_to_address",
        "arguments": {"address": 4096}
    })
    if res2 and "result" in res2:
        res3 = client.rpc("tools/call", {
            "name": "r2000_get_disassembly_cursor",
            "arguments": {}
        })
        if res3 and "result" in res3:
            addr_text = res3["result"].get("content", [{}])[0].get("text", "")
            print(f"  New cursor: {addr_text}")
            print("PASS" if "$1000" in addr_text else "FAIL: Cursor did not move to $1000")
        else:
            print("FAIL: Could not verify cursor after jump")
    elif res2 and "error" in res2:
        print(f"FAIL (jump error): {res2['error']['message']}")
    else:
        print(f"FAIL: {res2}")


def test_batch_execute(client):
    print("\nTesting r2000_batch_execute...")

    calls = [
        {
            "name": "r2000_set_label_name",
            "arguments": {"address": 0x1005, "name": "BATCH_LABEL"}
        },
        {
            "name": "r2000_set_comment",
            "arguments": {"address": 0x1005, "comment": "Batch Comment", "type": "side"}
        },
        {
            "name": "r2000_set_data_type",
            "arguments": {"start_address": 0x1000, "end_address": 0x100F, "data_type": "code"}
        },
    ]

    res = client.rpc("tools/call", {
        "name": "r2000_batch_execute",
        "arguments": {"calls": calls}
    })

    if res and "result" in res and "content" in res["result"]:
        content_text = res["result"]["content"][0]["text"]
        try:
            results = json.loads(content_text)
            if isinstance(results, list) and len(results) == len(calls):
                failed = [r for r in results if r.get("status") != "success"]
                if not failed:
                    print(f"PASS: All {len(calls)} batch calls succeeded")
                else:
                    print(f"FAIL: {len(failed)} call(s) failed: {failed}")
            else:
                print(f"FAIL: Expected {len(calls)} results, got: {results}")
        except Exception as e:
            print(f"FAIL: JSON parsing error: {e}")
    elif res and "error" in res:
        print(f"FAIL: {res['error']}")
    else:
        print(f"FAIL: No response: {res}")


def test_toggle_splitter(client):
    print("\nTesting r2000_toggle_splitter...")
    res = client.rpc("tools/call", {
        "name": "r2000_toggle_splitter",
        "arguments": {"address": 0x1010}
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        if "Splitter toggled" in text:
            print(f"PASS: {text}")
        else:
            print(f"FAIL: unexpected response: {text}")
    elif res and "error" in res:
        print(f"FAIL: {res['error']['message']}")
    else:
        print(f"FAIL: {res}")


def test_add_scope(client):
    print("\nTesting r2000_add_scope...")
    res = client.rpc("tools/call", {
        "name": "r2000_add_scope",
        "arguments": {"start_address": 0x1000, "end_address": 0x1010}
    })
    if res and "result" in res:
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        if "Added Scope" in text:
            print(f"PASS: {text}")
        else:
            print(f"FAIL: unexpected response: {text}")
    elif res and "error" in res:
        print(f"FAIL: {res['error']['message']}")
    else:
        print(f"FAIL: {res}")


def test_undo_redo(client):
    print("\nTesting r2000_undo / r2000_redo...")
    res = client.rpc("tools/call", {"name": "r2000_undo", "arguments": {}})
    if res and "result" in res:
        print(f"PASS (undo): {res['result'].get('content', [{}])[0].get('text','')}")
    else:
        print(f"FAIL (undo): {res}")

    res = client.rpc("tools/call", {"name": "r2000_redo", "arguments": {}})
    if res and "result" in res:
        print(f"PASS (redo): {res['result'].get('content', [{}])[0].get('text','')}")
    else:
        print(f"FAIL (redo): {res}")


def assert_error(res, description):
    """Assert that an MCP response contains an error (not a result). Prints PASS/FAIL."""
    if res and "error" in res:
        msg = res["error"].get("message", "")
        print(f"  PASS: {description!r} -> error: {msg!r}")
        return True
    elif res and "result" in res:
        # Some servers wrap errors inside content; check for that too.
        content = res["result"].get("content", [])
        text = content[0].get("text", "") if content else ""
        if "error" in text.lower() or "missing" in text.lower() or "invalid" in text.lower():
            print(f"  PASS (error in content): {description!r} -> {text[:80]!r}")
            return True
        print(f"  FAIL: {description!r} — expected error, got result: {text[:80]!r}")
        return False
    else:
        print(f"  FAIL: {description!r} — no response")
        return False


def test_malformed_calls(client):
    print("\nTesting malformed MCP calls (all should return errors)...")
    all_pass = True

    def check(res, description):
        nonlocal all_pass
        if not assert_error(res, description):
            all_pass = False

    def call(name, arguments):
        return client.rpc("tools/call", {"name": name, "arguments": arguments})

    # -----------------------------------------------------------------------
    # Unknown tool
    # -----------------------------------------------------------------------
    check(
        call("r2000_nonexistent_tool", {}),
        "unknown tool name"
    )

    # -----------------------------------------------------------------------
    # r2000_set_comment — the original silent-failure bug
    # -----------------------------------------------------------------------
    # Wrong key for 'comment' (used to be 'text' in agent calls)
    check(
        call("r2000_set_comment", {"address": 0x1000, "text": "hello", "type": "line"}),
        "r2000_set_comment: 'text' instead of 'comment'"
    )
    # Wrong key for 'type' (used to be 'comment_type' in agent calls)
    check(
        call("r2000_set_comment", {"address": 0x1000, "comment": "hello", "comment_type": "line"}),
        "r2000_set_comment: 'comment_type' instead of 'type'"
    )
    # Missing 'comment' entirely
    check(
        call("r2000_set_comment", {"address": 0x1000, "type": "line"}),
        "r2000_set_comment: missing 'comment'"
    )
    # Missing 'type' entirely
    check(
        call("r2000_set_comment", {"address": 0x1000, "comment": "hello"}),
        "r2000_set_comment: missing 'type'"
    )
    # Invalid 'type' enum value
    check(
        call("r2000_set_comment", {"address": 0x1000, "comment": "hello", "type": "inline"}),
        "r2000_set_comment: invalid 'type' value 'inline'"
    )
    # Missing 'address'
    check(
        call("r2000_set_comment", {"comment": "hello", "type": "line"}),
        "r2000_set_comment: missing 'address'"
    )

    # -----------------------------------------------------------------------
    # r2000_set_label_name
    # -----------------------------------------------------------------------
    check(
        call("r2000_set_label_name", {"name": "LABEL_NO_ADDR"}),
        "r2000_set_label_name: missing 'address'"
    )
    check(
        call("r2000_set_label_name", {"address": 0x1000}),
        "r2000_set_label_name: missing 'name'"
    )

    # -----------------------------------------------------------------------
    # r2000_set_data_type
    # -----------------------------------------------------------------------
    check(
        call("r2000_set_data_type", {"start_address": 0x1000, "end_address": 0x100F}),
        "r2000_set_data_type: missing 'data_type'"
    )
    check(
        call("r2000_set_data_type", {"start_address": 0x1000, "end_address": 0x100F, "data_type": "banana"}),
        "r2000_set_data_type: invalid 'data_type' value 'banana'"
    )
    check(
        call("r2000_set_data_type", {"end_address": 0x100F, "data_type": "code"}),
        "r2000_set_data_type: missing 'start_address'"
    )
    check(
        call("r2000_set_data_type", {"start_address": 0x100F, "end_address": 0x1000, "data_type": "code"}),
        "r2000_set_data_type: start > end"
    )

    # -----------------------------------------------------------------------
    # r2000_read_region
    # -----------------------------------------------------------------------
    check(
        call("r2000_read_region", {"end_address": 0x100F}),
        "r2000_read_region: missing 'start_address'"
    )
    check(
        call("r2000_read_region", {"start_address": 0x1000}),
        "r2000_read_region: missing 'end_address'"
    )
    check(
        call("r2000_read_region", {"start_address": 0x1000, "end_address": 0x100F, "view": "text"}),
        "r2000_read_region: invalid 'view' value 'text'"
    )

    # -----------------------------------------------------------------------
    # r2000_read_selected
    # -----------------------------------------------------------------------
    check(
        call("r2000_read_selected", {"view": "raw"}),
        "r2000_read_selected: invalid 'view' value 'raw'"
    )

    # -----------------------------------------------------------------------
    # r2000_set_operand_format
    # -----------------------------------------------------------------------
    check(
        call("r2000_set_operand_format", {"address": 0x1000}),
        "r2000_set_operand_format: missing 'format'"
    )
    check(
        call("r2000_set_operand_format", {"address": 0x1000, "format": "octal"}),
        "r2000_set_operand_format: invalid 'format' value 'octal'"
    )
    check(
        call("r2000_set_operand_format", {"format": "hex"}),
        "r2000_set_operand_format: missing 'address'"
    )

    # -----------------------------------------------------------------------
    # r2000_get_symbols
    # -----------------------------------------------------------------------
    check(
        call("r2000_get_symbols", {"kind": "admin"}),
        "r2000_get_symbols: invalid 'kind' value 'admin'"
    )
    check(
        call("r2000_get_symbols", {"start_address": 0x1000}),
        "r2000_get_symbols: start_address without end_address"
    )
    check(
        call("r2000_get_symbols", {"end_address": 0x100F}),
        "r2000_get_symbols: end_address without start_address"
    )
    check(
        call("r2000_get_symbols", {"start_address": 0x100F, "end_address": 0x1000}),
        "r2000_get_symbols: start > end"
    )

    # -----------------------------------------------------------------------
    # r2000_get_comments
    # -----------------------------------------------------------------------
    check(
        call("r2000_get_comments", {"type": "block"}),
        "r2000_get_comments: invalid 'type' value 'block'"
    )
    check(
        call("r2000_get_comments", {"start_address": 0x1000}),
        "r2000_get_comments: start_address without end_address"
    )
    check(
        call("r2000_get_comments", {"end_address": 0x100F}),
        "r2000_get_comments: end_address without start_address"
    )
    check(
        call("r2000_get_comments", {"start_address": 0x100F, "end_address": 0x1000}),
        "r2000_get_comments: start > end"
    )

    # -----------------------------------------------------------------------
    # r2000_get_address_details / r2000_get_cross_references / r2000_jump_to_address
    # -----------------------------------------------------------------------
    check(
        call("r2000_get_address_details", {}),
        "r2000_get_address_details: missing 'address'"
    )
    check(
        call("r2000_get_cross_references", {}),
        "r2000_get_cross_references: missing 'address'"
    )
    check(
        call("r2000_jump_to_address", {}),
        "r2000_jump_to_address: missing 'address'"
    )

    # -----------------------------------------------------------------------
    # r2000_toggle_splitter / r2000_add_scope
    # -----------------------------------------------------------------------
    check(
        call("r2000_toggle_splitter", {}),
        "r2000_toggle_splitter: missing 'address'"
    )
    check(
        call("r2000_add_scope", {"start_address": 0x1000}),
        "r2000_add_scope: missing 'end_address'"
    )
    check(
        call("r2000_add_scope", {"end_address": 0x1010}),
        "r2000_add_scope: missing 'start_address'"
    )
    check(
        call("r2000_add_scope", {"start_address": 0x1010, "end_address": 0x1000}),
        "r2000_add_scope: start > end"
    )

    # -----------------------------------------------------------------------
    # r2000_search_memory
    # -----------------------------------------------------------------------
    check(
        call("r2000_search_memory", {}),
        "r2000_search_memory: missing 'query'"
    )

    # -----------------------------------------------------------------------
    # r2000_search_disassembly
    # -----------------------------------------------------------------------
    check(
        call("r2000_search_disassembly", {}),
        "r2000_search_disassembly: missing 'query'"
    )

    # -----------------------------------------------------------------------
    # r2000_batch_execute
    # -----------------------------------------------------------------------
    check(
        call("r2000_batch_execute", {}),
        "r2000_batch_execute: missing 'calls'"
    )

    # -----------------------------------------------------------------------
    # tools/call with missing 'name'
    # -----------------------------------------------------------------------
    res = client.rpc("tools/call", {"arguments": {}})
    check(res, "tools/call: missing 'name' field")

    if all_pass:
        print("PASS: All malformed calls correctly returned errors.")
    else:
        print("FAIL: Some malformed calls did not return the expected error.")


if __name__ == "__main__":
    client = MCPClient()
    client.start()

    test_list_tools(client)
    test_list_resources(client)
    test_set_label(client)
    test_set_comment(client)
    test_set_data_type(client)
    test_read_region(client)
    test_read_selected(client)
    test_get_binary_info(client)
    test_get_disassembly_cursor(client)
    test_jump_to_address(client)
    test_misc_tools(client)
    test_search_disassembly(client)
    test_toggle_splitter(client)
    test_add_scope(client)
    test_undo_redo(client)
    test_batch_execute(client)
    test_malformed_calls(client)
