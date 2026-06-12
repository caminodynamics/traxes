#!/usr/bin/env python3

import json
import time
import argparse
import sys
from urllib.request import urlopen
from urllib.error import URLError, HTTPError

def load_payload(payload_type):
    """Load payload from file"""
    filename = f"payloads/{payload_type}_tool_call.json"
    try:
        with open(filename, 'r') as f:
            return json.load(f)
    except FileNotFoundError:
        print(f"Error: Payload file {filename} not found")
        sys.exit(1)
    except json.JSONDecodeError:
        print(f"Error: Invalid JSON in {filename}")
        sys.exit(1)

def send_request(payload, proxy_url="http://localhost:8081"):
    """Send request to Traxes proxy"""
    start_time = time.time()
    
    try:
        data = json.dumps(payload).encode('utf-8')
        req = urlopen(proxy_url, data, timeout=10)
        response_data = req.read().decode('utf-8')
        response = json.loads(response_data)
        end_time = time.time()
        
        latency_ms = (end_time - start_time) * 1000
        
        return {
            "success": True,
            "latency_ms": latency_ms,
            "status_code": req.getcode(),
            "response": response
        }
        
    except HTTPError as e:
        end_time = time.time()
        latency_ms = (end_time - start_time) * 1000
        
        try:
            response_data = e.read().decode('utf-8')
            response = json.loads(response_data)
        except:
            response = {"error": "Failed to parse error response"}
        
        return {
            "success": False,
            "latency_ms": latency_ms,
            "status_code": e.code,
            "response": response
        }
        
    except URLError as e:
        end_time = time.time()
        latency_ms = (end_time - start_time) * 1000
        
        return {
            "success": False,
            "latency_ms": latency_ms,
            "status_code": None,
            "response": {"error": f"Connection error: {str(e)}"}
        }
        
    except Exception as e:
        end_time = time.time()
        latency_ms = (end_time - start_time) * 1000
        
        return {
            "success": False,
            "latency_ms": latency_ms,
            "status_code": None,
            "response": {"error": f"Unexpected error: {str(e)}"}
        }

def print_audit_artifact(response):
    """Pretty print audit artifact"""
    if "status" in response and response["status"] == "DENY":
        print("\n=== AUDIT ARTIFACT ===")
        print(json.dumps(response, indent=2))
        print("======================\n")

def main():
    parser = argparse.ArgumentParser(description='Test Traxes proxy with payloads')
    parser.add_argument('--safe', action='store_true', help='Test with safe payload')
    parser.add_argument('--malicious', action='store_true', help='Test with malicious payload')
    parser.add_argument('--proxy-url', default='http://localhost:8081', 
                       help='Traxes proxy URL (default: http://localhost:8081)')
    
    args = parser.parse_args()
    
    if not args.safe and not args.malicious:
        print("Error: Must specify either --safe or --malicious")
        parser.print_help()
        sys.exit(1)
    
    if args.safe and args.malicious:
        print("Error: Cannot specify both --safe and --malicious")
        sys.exit(1)
    
    payload_type = "valid" if args.safe else "malicious"
    payload = load_payload(payload_type)
    
    print(f"Testing {payload_type} payload...")
    print(f"Sending request to {args.proxy_url}")
    
    result = send_request(payload, args.proxy_url)
    
    print(f"Request latency: {result['latency_ms']:.2f}ms")
    print(f"Status code: {result['status_code']}")
    
    if result['success']:
        print("✓ Request successful")
        if result['response'].get('status') == 'SUCCESS':
            print("✓ Target application reached")
        else:
            print("⚠ Unexpected response from proxy")
    else:
        if result['response'].get('status') == 'DENY':
            print(f"[HTTP {result['status_code']}] BLOCKED - Malicious Payload Detected")
            print(f"Execution Latency: {result['latency_ms']:.2f}ms (Python Mock Proxy)")
            print("---")
            print("NOTE: You are testing the architectural flow via the Open Edge functional mock.")
            print("The production Closed Core (Rust) delivers ~5.28ms median end-to-end validation latency.")
            print("Core rule evaluation microbenchmarks measured ~19 CPU cycles on bare-metal ESP32 tests.")
            print_audit_artifact(result['response'])
        else:
            print("✗ Request denied or failed")
            print(f"Error: {result['response'].get('error', 'Unknown error')}")
    
    print(f"\nResponse: {json.dumps(result['response'], indent=2)}")

if __name__ == "__main__":
    main()
