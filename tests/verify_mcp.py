import requests
import json
import sys
import time
import threading

BASE_URL = "http://localhost:3000"


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
        
        # 1. Initialize via POST (Capture Session ID and Start SSE Listener)
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
             # Initial POST request to establish session and get the stream
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
            
            # Start listening to the stream from THIS response
            self.read_thread = threading.Thread(target=self._listen_sse, args=(response,), daemon=True)
            self.read_thread.start()
            
            # Wait for initialization response
            # Since we sent the initialize request in the POST, the response will come via the SSE stream
            # We need to wait for it.
            if self._wait_for_response(1):
                 print("Initialized successfully.")
                 # Send initialized notification (using the now-established session)
                 self.rpc("notifications/initialized", {}) 
            else:
                 print("Timeout waiting for initialization response")
                 sys.exit(1)

        except Exception as e:
            print(f"Connection failed: {e}")
            sys.exit(1)

    def _listen_sse(self, initial_response):
        response = initial_response
        
        while True:
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
                            print(f"DEBUG: SSE Data received: {data[:100]}...")
                            try:
                                msg = json.loads(data)
                                self._handle_message(msg)
                            except json.JSONDecodeError:
                                print(f"Failed to decode JSON: {data}")
            except Exception as e:
                print(f"SSE stream error: {e}")
            finally:
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
            # Send request on the same endpoint, using the Session ID
            print(f"DEBUG: Sending {method} with headers: {headers}")
            response = requests.post(BASE_URL, json=payload, headers=headers, timeout=5)
            print(f"DEBUG: Response status: {response.status_code}")
            response.raise_for_status()
            
            # The response usually comes via the SSE stream (the initial connection)
            # But technically for "Accepted" requests it might just be 202. 
            # We wait for the JSON-RPC response in the stream.
            
            res = self._wait_for_response(current_id)
            if res:
                return res
            
            if method.startswith("notifications/"):
                 return None

            print(f"Timeout waiting for response to {method}")
            return None
            
        except Exception as e:
            print(f"Request failed: {e}")
            if hasattr(e, 'response') and e.response is not None:
                 print(f"Response content: {e.response.text}")
            return None

def test_list_tools(client):
    print("\nTesting tools/list...")
    res = client.rpc("tools/list")
    if res and "result" in res:
        tools = res["result"]["tools"]
        print(f"Found {len(tools)} tools.")
        names = [t["name"] for t in tools]
        print(f"Tools: {names}")
        if "set_label_name" in names and "convert_region_to_code" in names:
            print("PASS")
        else:
            print("FAIL: Missing tools")
    else:
        print(f"FAIL: {res}")

def test_set_label(client):
    print("\nTesting set_label_name...")
    # Assuming standard C64 load address $0801 (2049) is valid data
    # We'll try to set a label at $1000 (4096)
    res = client.rpc("tools/call", {
        "name": "set_label_name",
        "arguments": {
            "address": 4096,
            "name": "TEST_LABEL"
        }
    })
    if res and "result" in res:
        print("Success:", res["result"])
        print("PASS")
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
        "name": "convert_region_to_code",
        "arguments": {
            "start_address": 0x1000,
            "end_address": 0x100F
        }
    })
    print(res.get("result", res))

    # 2. Set $1010-$101f to BYTES
    print("- Converting $1010-$101F to BYTES")
    res = client.rpc("tools/call", {
        "name": "convert_region_to_bytes",
        "arguments": {
            "start_address": 0x1010,
            "end_address": 0x101F
        }
    })
    print(res.get("result", res))

    # 3. Set $1020-$102f to WORDS
    print("- Converting $1020-$102F to WORDS")
    res = client.rpc("tools/call", {
        "name": "convert_region_to_words",
        "arguments": {
            "start_address": 0x1020,
            "end_address": 0x102F
        }
    })
    print(res.get("result", res))

    # 4. Set Line Comment at $1000
    print("- Setting line comment at $1000")
    res = client.rpc("tools/call", {
        "name": "set_line_comment",
        "arguments": {
            "address": 0x1000,
            "comment": "added by MCP"
        }
    })
    print(res.get("result", res))

def test_read_resource(client):
    print("\nTesting read_resource disasm://4096-4097...")
    # Read resource
    res = client.rpc("resources/read", {
        "uri": "disasm://region/4096/4097"
    })
    if res and "result" in res:
        print("Success:")
        print(json.dumps(res["result"], indent=2))
        print("PASS")
    else:
        print(f"FAIL: {res}")

def test_read_selected_resources(client):
    print("\nTesting read_resource disasm://selected...")
    res = client.rpc("resources/read", {"uri": "disasm://selected"})
    if res and "result" in res:
        print("Success (Disasm):")
        print(json.dumps(res["result"], indent=2))
        print("PASS (Disasm) - Verify content manually")
    else:
        print(f"FAIL (Disasm): {res}")

    print("\nTesting read_resource hexdump://selected...")
    res = client.rpc("resources/read", {"uri": "hexdump://selected"})
    if res and "result" in res:
        print("Success (Hexdump):")
        print(json.dumps(res["result"], indent=2))
        print("PASS (Hexdump) - Verify content manually")
    else:
        print(f"FAIL (Hexdump): {res}")

def test_convert_lo_hi_address(client):
    print("\nTesting convert_region_to_lo_hi_address...")
    # Using a dummy range, assuming it won't crash even if data is random
    res = client.rpc("tools/call", {
        "name": "convert_region_to_lo_hi_address",
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

if __name__ == "__main__":
    client = MCPClient()
    client.start()
    
    test_list_tools(client)
    test_set_label(client)
    test_complex_scenario(client)
    test_read_resource(client)
    test_read_selected_resources(client)
    test_convert_lo_hi_address(client)
