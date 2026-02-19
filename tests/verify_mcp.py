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
                "protocolVersion": "2024-11-05", # Example version
                "capabilities": {},
                "clientInfo": {
                    "name": "verify_mcp",
                    "version": "1.0"
                }
            },
            "id": 1
        }

        try:
             # Initial POST request to establish session and get initialization response
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

            # Check for Session ID
            self.session_id = response.headers.get("mcp-session-id")
            if not self.session_id:
                print("Error: No mcp-session-id in response headers")
                sys.exit(1)

            print(f"Connected. Session ID: {self.session_id}")

            # Read the initialization response from the POST stream
            init_response = None
            for line in response.iter_lines():
                if line:
                    decoded_line = line.decode('utf-8')
                    if decoded_line.startswith("data:"):
                        data = decoded_line[5:].strip()
                        if data and data != '':
                            try:
                                msg = json.loads(data)
                                if msg.get("id") == 1:  # Our initialization response
                                    init_response = msg
                                    break
                            except json.JSONDecodeError:
                                continue

            # Close the POST response stream
            response.close()

            if not init_response or "result" not in init_response:
                print("Failed to get initialization response")
                sys.exit(1)

            print("Initialized successfully.")

            # 2. Open a dedicated GET stream for ongoing communication
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

            # Start listening to the GET stream
            self.read_thread = threading.Thread(target=self._listen_sse, args=(get_response,), daemon=True)
            self.read_thread.start()

            # Wait for connection to be established
            if not self.connected.wait(timeout=5):
                print("Timeout waiting for GET stream connection")
                sys.exit(1)

            # Send initialized notification
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
                             # We can ignore event types for now in this simple client
                            continue
                        if decoded_line.startswith("data:"):
                            data = decoded_line[5:].strip()
                            if not data:
                                continue
                            # print(f"DEBUG: SSE Data received: {data[:100]}...")
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
            time.sleep(1) # Wait a bit before reconnecting

            if not self.session_id:
                print("No session ID, cannot reconnect.")
                break

            try:
                headers={
                    "Accept": "text/event-stream",
                    "mcp-session-id": self.session_id
                }
                print(f"Reconnecting to {BASE_URL} with session {self.session_id}")
                response = requests.get(
                    BASE_URL,
                    headers=headers,
                    stream=True,
                    timeout=None # Keep open indefinitely
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
             # Handle notifications or server requests if needed
             # print(f"Notification: {msg['method']}")
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
            # Send request on the /message endpoint, using the Session ID
            cmd_url = BASE_URL
            # print(f"DEBUG: Sending {method} to {cmd_url} with headers: {headers}")
            response = requests.post(cmd_url, json=payload, headers=headers, stream=True, timeout=10)

            if response.status_code == 200:
                # If it's a stream, we should read it
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
                                            # Found our response
                                            response.close()
                                            return msg
                                        else:
                                            # Might be a notification or something else, handle it
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
                # 202 Accepted means the response WILL come via the SSE stream
                pass
            else:
                 print(f"Request failed with status {response.status_code}: {response.text}")
                 response.raise_for_status()

            if is_notification:
                response.close()
                return None

            # Fallback: wait for the response in the shared stream if not found in POST stream
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


def test_list_tools(client):
    print("\nTesting tools/list...")
    res = client.rpc("tools/list")
    if res and "result" in res:
        tools = res["result"]["tools"]
        print(f"Found {len(tools)} tools.")
        names = [t["name"] for t in tools]
        print(f"Tools: {names}")
        if "r2000_set_label_name" in names and "r2000_convert_region_to_code" in names:
            print("PASS")
        else:
            print("FAIL: Missing tools")
    else:
        print(f"FAIL: {res}")

def test_set_label(client):
    print("\nTesting r2000_set_label_name...")
    # Assuming standard C64 load address $0801 (2049) is valid data
    # We'll try to set a label at $1000 (4096)
    res = client.rpc("tools/call", {
        "name": "r2000_set_label_name",
        "arguments": {
            "address": 4096,
            "name": "TEST_LABEL"
        }
    })
    if res and "result" in res:
        print("Success:", res["result"])
        # Verify content regarding "text"
        if "content" in res["result"] and len(res["result"]["content"]) > 0:
             text_content = res["result"]["content"][0].get("text", "")
             print(f"Tool output: {text_content}")
             if "Label set at" in text_content:
                 print("PASS: Tool response confirms action")
             else:
                 print("FAIL: Tool response missing confirmation text")
        else:
             print("FAIL: Tool response missing 'content' field")
    elif res and "error" in res:
        # It might fail if address is out of bounds, but let's see
        print("Error:", res["error"])
    else:
        print(f"FAIL: {res}")

def test_complex_scenario(client):
    print("\nTesting complex scenario...")

    # 1. Set $1000-$100f to CODE
    print("- Converting $1000-$100F to CODE")
    res = client.rpc("tools/call", {
        "name": "r2000_convert_region_to_code",
        "arguments": {
            "start_address": 0x1000,
            "end_address": 0x100F
        }
    })
    if res:
        print(res.get("result", res))
    else:
        print("FAIL: No response")

    # 2. Set $1010-$101f to BYTES
    print("- Converting $1010-$101F to BYTES")
    res = client.rpc("tools/call", {
        "name": "r2000_convert_region_to_bytes",
        "arguments": {
            "start_address": 0x1010,
            "end_address": 0x101F
        }
    })
    if res:
        print(res.get("result", res))
    else:
        print("FAIL: No response")

    # 3. Set $1020-$102f to WORDS
    print("- Converting $1020-$102F to WORDS")
    res = client.rpc("tools/call", {
        "name": "r2000_convert_region_to_words",
        "arguments": {
            "start_address": 0x1020,
            "end_address": 0x102F
        }
    })
    if res:
         print(res.get("result", res))
    else:
         print("FAIL: No response")

    # 4. Set Line Comment at $1000
    print("- Setting line comment at $1000")
    res = client.rpc("tools/call", {
        "name": "r2000_set_line_comment",
        "arguments": {
            "address": 0x1000,
            "comment": "added by MCP"
        }
    })
    if res:
        print(res.get("result", res))
    else:
        print("FAIL: No response")



def test_convert_lo_hi_address(client):
    print("\nTesting r2000_convert_region_to_lo_hi_address...")
    # Using a dummy range, assuming it won't crash even if data is random
    res = client.rpc("tools/call", {
        "name": "r2000_convert_region_to_lo_hi_address",
        "arguments": {
            "start_address": 0x1000,
            "end_address": 0x1003
        }
    })
    if res and "result" in res:
        print("Success (Lo/Hi Addr):")
        print(json.dumps(res["result"], indent=2))
        print("PASS (Lo/Hi Addr)")
    else:
        print(f"FAIL (Lo/Hi Addr): {res}")

def test_tool_response_content(client):
    print("\nTesting tool response content (r2000_set_line_comment)...")
    res = client.rpc("tools/call", {
        "name": "r2000_set_line_comment",
        "arguments": {
            "address": 0x1000,
            "comment": "test comment"
        }
    })

    if res and "result" in res:
        content = res["result"].get("content", [])
        if content and content[0].get("text"):
            print(f"Response text: {content[0]['text']}")
            if "Comment set at" in content[0]['text']:
                print("PASS: Tool returned 'applied' confirmation")
            else:
                print("FAIL: Tool did not return expected confirmation text")
        else:
            print("FAIL: Empty or invalid content in tool response")
    else:
        print(f"FAIL: Tool call failed {res}")

def test_read_disasm_region(client):
    print("\nTesting r2000_read_disasm_region (formerly a resource)...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_disasm_region",
        "arguments": {
            "start_address": 4096,
            "end_address": 4097
        }
    })
    if res and "result" in res:
        print("Success:")
        print(json.dumps(res["result"], indent=2))

        # Verify content
        if "content" in res["result"] and len(res["result"]["content"]) > 0:
            content = res["result"]["content"][0]
            if "text" in content and len(content["text"]) > 0:
                 print("PASS: Tool returned content")
                 print(f"Content snippet: {content['text'][:50]}...")
            else:
                 print("FAIL: Tool content 'text' is empty")
        else:
             print("FAIL: Tool response missing 'content'")

    else:
        print(f"FAIL: {res}")

def test_read_hexdump_region(client):
    print("\nTesting r2000_read_hexdump_region...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_hexdump_region",
        "arguments": {
            "start_address": 4096,
            "end_address": 4097
        }
    })
    if res and "result" in res:
        # print("Success:")
        # print(json.dumps(res["result"], indent=2))
        if "content" in res["result"] and len(res["result"]["content"]) > 0:
             print("PASS: r2000_read_hexdump_region returned content")
        else:
             print("FAIL: r2000_read_hexdump_region response missing content")
    else:
        print(f"FAIL: {res}")

def test_read_selected_tools(client):
    print("\nTesting r2000_read_selected_disasm...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_selected_disasm",
        "arguments": {}
    })
    if res and "result" in res:
        print("Success (Disasm):")
        print(json.dumps(res["result"], indent=2))
        print("PASS (Disasm) - Verify content manually")
    else:
        print(f"FAIL (Disasm): {res}")

    print("\nTesting r2000_read_selected_hexdump...")
    res = client.rpc("tools/call", {
        "name": "r2000_read_selected_hexdump",
        "arguments": {}
    })
    if res and "result" in res:
        print("Success (Hexdump):")
        print(json.dumps(res["result"], indent=2))
        print("PASS (Hexdump) - Verify content manually")
    else:
        print(f"FAIL (Hexdump): {res}")

def test_new_tools(client):
    print("\nTesting new tools (search, xrefs, symbols, comments)...")

    # Search Memory
    print("- r2000_search_memory")
    res = client.rpc("tools/call", {
        "name": "r2000_search_memory",
        "arguments": {
            "query": "A9 00" # lda #$00
        }
    })
    if res and "result" in res:
         print("PASS: r2000_search_memory returned result")
    else:
         print(f"FAIL: r2000_search_memory {res}")

    # Get Cross References
    print("- r2000_get_cross_references")
    res = client.rpc("tools/call", {
        "name": "r2000_get_cross_references",
        "arguments": {
            "address": 0x1000
        }
    })
    if res and "result" in res:
         print("PASS: r2000_get_cross_references returned result")
    else:
         print(f"FAIL: r2000_get_cross_references {res}")

    # Get Symbol Table
    print("- r2000_get_symbol_table")
    res = client.rpc("tools/call", {
        "name": "r2000_get_symbol_table",
        "arguments": {}
    })
    if res and "result" in res:
         print("PASS: r2000_get_symbol_table returned result")
    else:
         print(f"FAIL: r2000_get_symbol_table {res}")

    # Get All Comments
    print("- r2000_get_all_comments")
    res = client.rpc("tools/call", {
        "name": "r2000_get_all_comments",
        "arguments": {}
    })
    if res and "result" in res:
         print("PASS: r2000_get_all_comments returned result")
         try:
            content_text = res["result"]["content"][0]["text"]
            comments = json.loads(content_text)
            if isinstance(comments, list):
                print(f"PASS: Comments is a list (len={len(comments)})")
                if len(comments) > 0:
                    c = comments[0]
                    if "address" in c and "type" in c and "comment" in c:
                         print(f"PASS: Comment structure matches: {c}")
                    else:
                         print(f"FAIL: Invalid comment structure: {c}")
            else:
                print("FAIL: Comments is not a list")
         except Exception as e:
             print(f"FAIL: Error parsing comments: {e}")
    else:
         print(f"FAIL: r2000_get_all_comments {res}")

    # Set Operand Format
    print("- r2000_set_operand_format")
    res = client.rpc("tools/call", {
        "name": "r2000_set_operand_format",
        "arguments": {
            "address": 0x1000,
            "format": "binary"
        }
    })
    if res and "result" in res:
         print("PASS: r2000_set_operand_format returned result")
    else:
         print(f"FAIL: r2000_set_operand_format {res}")

    # Save Project
    print("- r2000_save_project")
    res = client.rpc("tools/call", {
        "name": "r2000_save_project",
        "arguments": {}
    })
    # We expect an error because no project is loaded, but that confirms the tool is reachable
    if res and "error" in res:
         print(f"PASS: r2000_save_project returned expected error (no project loaded): {res['error']['message']}")
    elif res and "result" in res:
         print("PASS: r2000_save_project returned result (unexpected but valid)")
    else:
         print(f"FAIL: r2000_save_project {res}")


def test_list_resources(client):
    print("\nTesting resources/list...")
    res = client.rpc("resources/list")
    if res and "result" in res:
        resources = res["result"]["resources"]
        print(f"Found {len(resources)} resources.")
        uris = [r["uri"] for r in resources]
        print(f"Resources: {uris}")
        if "disasm://main" in uris and "binary://main" in uris:
             print("PASS")
        else:
             print("FAIL: Missing expected resources")
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
        if content and content[0].get("text"):
            text = content[0]["text"]
            print(f"Response text: {text}")
            try:
                data = json.loads(text)
                if "origin" in data and "size" in data and "platform" in data:
                    print("PASS: Returned origin, size, and platform")
                    print(f"Platform: {data['platform']}")
                else:
                    print(f"FAIL: Missing fields in response: {data.keys()}")
            except json.JSONDecodeError:
                 print(f"FAIL: Could not decode JSON response: {text}")
        else:
             print("FAIL: Check content structure")
    else:
        print(f"FAIL: {res}")


def test_get_disassembly_cursor(client):
    print("\nTesting r2000_get_disassembly_cursor...")
    res = client.rpc("tools/call", {
        "name": "r2000_get_disassembly_cursor",
        "arguments": {}
    })
    if res and "result" in res:
        print("Success:")
        print(json.dumps(res["result"], indent=2))

        # Verify content
        if "content" in res["result"] and len(res["result"]["content"]) > 0:
            content = res["result"]["content"][0]
            if "text" in content:
                 print(f"PASS: Tool returned address: {content['text']}")
            else:
                 print("FAIL: Tool content 'text' is empty")
        else:
             print("FAIL: Tool response missing 'content'")

    elif res and "error" in res:
         # Acceptable if cursor is out of bounds (e.g. empty project)
         print(f"PASS (valid error): {res['error']['message']}")
    else:
        print(f"FAIL: {res}")


def test_jump_to_address(client):
    print("\nTesting r2000_jump_to_address...")

    # 1. Get current
    print("- Getting current cursor...")
    res1 = client.rpc("tools/call", {
        "name": "r2000_get_disassembly_cursor",
        "arguments": {}
    })

    if res1 and "result" in res1 and "content" in res1["result"]:
        content = res1["result"]["content"]
        if content and len(content) > 0:
             print(f"Current: {content[0]['text']}")

    # 2. Jump to $1000
    print("- Jumping to $1000...")
    res2 = client.rpc("tools/call", {
        "name": "r2000_jump_to_address",
        "arguments": {
            "address": 4096
        }
    })

    if res2 and "result" in res2:
         content = res2["result"].get("content", [])
         print(f"Jump Result: {json.dumps(content, indent=2)}")

         # 3. Verify new cursor
         res3 = client.rpc("tools/call", {
            "name": "r2000_get_disassembly_cursor",
            "arguments": {}
         })

         if res3 and "result" in res3 and "content" in res3["result"]:
             addr_text = res3["result"]["content"][0]["text"]
             print(f"New Cursor: {addr_text}")
             if "$1000" in addr_text:
                 print("PASS: Cursor moved successfully")
             else:
                 print("FAIL: Cursor did not move to $1000")
         else:
             print("FAIL: Could not verify cursor after jump")

    elif res2 and "error" in res2:
         print(f"FAIL (Jump Error): {res2['error']['message']}")
    else:
         print(f"FAIL (No Response): {res2}")

def test_batch_execute(client):
    print("\nTesting r2000_batch_execute...")

    # 1. Prepare batch calls
    calls = [
        {
            "name": "r2000_set_label_name",
            "arguments": {
                "address": 0x1005,
                "name": "BATCH_LABEL"
            }
        },
        {
            "name": "r2000_set_side_comment",
            "arguments": {
                "address": 0x1005,
                "comment": "Batch Comment"
            }
        }
    ]

    res = client.rpc("tools/call", {
        "name": "r2000_batch_execute",
        "arguments": {
            "calls": calls
        }
    })

    if res and "result" in res and "content" in res["result"]:
        content_text = res["result"]["content"][0]["text"]
        print(f"Batch Result Text: {content_text}")
        try:
            results = json.loads(content_text)
            if isinstance(results, list) and len(results) == 2:
                if results[0].get("status") == "success" and results[1].get("status") == "success":
                    print("PASS: Batch execution success")
                else:
                    print(f"FAIL: Batch execution items failed: {results}")
            else:
                 print(f"FAIL: Invalid batch result structure: {results}")
        except Exception as e:
            print(f"FAIL: JSON parsing error: {e}")

    elif res and "error" in res:
        print(f"FAIL: Batch tool error: {res['error']}")
    else:
        print(f"FAIL: No response or invalid response {res}")

if __name__ == "__main__":
    client = MCPClient()
    client.start()

    test_list_tools(client)
    test_list_resources(client)
    test_set_label(client)
    test_complex_scenario(client)
    test_read_disasm_region(client)
    test_read_hexdump_region(client)
    test_read_selected_tools(client)
    test_convert_lo_hi_address(client)
    test_tool_response_content(client)
    test_new_tools(client)
    test_get_binary_info(client)
    test_get_disassembly_cursor(client)
    test_jump_to_address(client)
    test_batch_execute(client)
