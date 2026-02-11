import requests
import json
import sys

URL = "http://localhost:3000/jsonrpc"

def rpc(method, params={}):
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    }
    try:
        response = requests.post(URL, json=payload, timeout=5)
        response.raise_for_status()
        return response.json()
    except Exception as e:
        print(f"Request failed: {e}")
        return None

def test_list_tools():
    print("Testing tools/list...")
    res = rpc("tools/list")
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

def test_set_label():
    print("\nTesting set_label_name...")
    # Assuming standard C64 load address $0801 (2049) is valid data
    # We'll try to set a label at $1000 (4096)
    res = rpc("tools/call", {
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

def test_convert_region():
    print("\nTesting convert_region_to_bytes...")
    res = rpc("tools/call", {
        "name": "convert_region_to_bytes",
        "arguments": {
            "start_address": 4096,
            "end_address": 4100
        }
    })
    if res and "result" in res:
        print("Success:", res["result"])
        print("PASS")
    else:
        print(f"FAIL: {res}")

def test_complex_scenario():
    print("\nTesting complex scenario...")
    
    # 1. Set $1000-$100f to CODE
    print("- Converting $1000-$100F to CODE")
    res = rpc("tools/call", {
        "name": "convert_region_to_code",
        "arguments": {
            "start_address": 0x1000,
            "end_address": 0x100F
        }
    })
    print(res.get("result", res))

    # 2. Set $1010-$101f to BYTES
    print("- Converting $1010-$101F to BYTES")
    res = rpc("tools/call", {
        "name": "convert_region_to_bytes",
        "arguments": {
            "start_address": 0x1010,
            "end_address": 0x101F
        }
    })
    print(res.get("result", res))

    # 3. Set $1020-$102f to WORDS
    print("- Converting $1020-$102F to WORDS")
    res = rpc("tools/call", {
        "name": "convert_region_to_words",
        "arguments": {
            "start_address": 0x1020,
            "end_address": 0x102F
        }
    })
    print(res.get("result", res))

    # 4. Set Line Comment at $1000
    print("- Setting line comment at $1000")
    res = rpc("tools/call", {
        "name": "set_line_comment",
        "arguments": {
            "address": 0x1000,
            "comment": "added by MCP"
        }
    })
    print(res.get("result", res))

def test_read_resource():
    print("\nTesting read_resource disasm://4096-4097...")
    # Read resource
    res = rpc("resources/read", {
        "uri": "disasm://region/4096/4097"
    })
    if res and "result" in res:
        print("Success:")
        print(json.dumps(res["result"], indent=2))
        print("PASS")
    else:
        print(f"FAIL: {res}")

def test_read_selected_resources():
    print("\nTesting read_resource disasm://selected...")
    res = rpc("resources/read", {"uri": "disasm://selected"})
    if res and "result" in res:
        print("Success (Disasm):")
        print(json.dumps(res["result"], indent=2))
        print("PASS (Disasm) - Verify content manually")
    else:
        print(f"FAIL (Disasm): {res}")

    print("\nTesting read_resource hexdump://selected...")
    res = rpc("resources/read", {"uri": "hexdump://selected"})
    if res and "result" in res:
        print("Success (Hexdump):")
        print(json.dumps(res["result"], indent=2))
        print("PASS (Hexdump) - Verify content manually")
    else:
        print(f"FAIL (Hexdump): {res}")

def test_convert_lo_hi_address():
    print("\nTesting convert_region_to_lo_hi_address...")
    # Using a dummy range, assuming it won't crash even if data is random
    res = rpc("tools/call", {
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
    print(f"Connecting to {URL}")
    test_list_tools()
    test_set_label()
    # test_convert_region() # Replaced by complex scenario
    test_complex_scenario()
    test_read_resource()
    test_read_selected_resources()
    test_convert_lo_hi_address()
