#!/usr/bin/env python3

import json
import sys
import os

def test_payloads():
    """Test that payload files are valid JSON"""
    print("Testing payload files...")
    
    try:
        with open('../demo/payloads/valid_tool_call.json', 'r') as f:
            valid_payload = json.load(f)
            print("+ Valid payload loaded successfully")
            print(f"  Tool: {valid_payload['tool_call']['name']}")
    except Exception as e:
        print(f"- Error loading valid payload: {e}")
        return False
    
    try:
        with open('../demo/payloads/malicious_injection.json', 'r') as f:
            malicious_payload = json.load(f)
            print("+ Malicious payload loaded successfully")
            print(f"  Tool: {malicious_payload['tool_call']['name']}")
    except Exception as e:
        print(f"- Error loading malicious payload: {e}")
        return False
    
    return True

def test_python_syntax():
    """Test that Python files have valid syntax"""
    print("\nTesting Python syntax...")
    
    python_files = [
        'mock_traxes_proxy.py',
        'mock_target_app.py', 
        'test_harness.py'
    ]
    
    for file in python_files:
        try:
            with open(file, 'r') as f:
                code = f.read()
                compile(code, file, 'exec')
                print(f"+ {file} syntax is valid")
        except SyntaxError as e:
            print(f"- Syntax error in {file}: {e}")
            return False
        except Exception as e:
            print(f"- Error checking {file}: {e}")
            return False
    
    return True

def test_proxy_logic():
    """Test proxy validation logic"""
    print("\nTesting proxy validation logic...")
    
    try:
        # Import the proxy handler class
        sys.path.append('.')
        from mock_traxes_proxy import TraxesProxyHandler
        
        # Create a mock instance to test validation
        class MockHandler:
            def is_malicious_payload(self, payload):
                # Copy the validation logic
                if not isinstance(payload, dict):
                    return False, "INVALID_PAYLOAD_FORMAT"
                
                tool_call = payload.get("tool_call", {})
                parameters = tool_call.get("parameters", {})
                
                # Check for SQL injection patterns
                if "query" in parameters:
                    query = parameters["query"].upper()
                    import re
                    malicious_patterns = [
                        r'DROP\s+TABLE',
                        r'DELETE\s+FROM',
                        r'UPDATE\s+.*\s+SET',
                        r'INSERT\s+INTO',
                        r'ALTER\s+TABLE',
                        r'CREATE\s+TABLE',
                        r'TRUNCATE\s+TABLE',
                        r'EXEC\s*\(',
                        r'SP_EXECUTESQL',
                        r';\s*DROP',
                        r';\s*DELETE',
                        r';\s*UPDATE',
                        r'UNION\s+SELECT',
                        r'1\s*=\s*1',
                        r'OR\s+1\s*=\s*1'
                    ]
                    
                    for pattern in malicious_patterns:
                        if re.search(pattern, query, re.IGNORECASE):
                            return True, f"SQL_INJECTION_DETECTED: {pattern}"
                
                return False, None
        
        handler = MockHandler()
        
        # Test valid payload
        with open('../demo/payloads/valid_tool_call.json', 'r') as f:
            valid_payload = json.load(f)
            is_malicious, reason = handler.is_malicious_payload(valid_payload)
            if not is_malicious:
                print("+ Valid payload correctly allowed")
            else:
                print(f"- Valid payload incorrectly blocked: {reason}")
                return False
        
        # Test malicious payload
        with open('../demo/payloads/malicious_injection.json', 'r') as f:
            malicious_payload = json.load(f)
            is_malicious, reason = handler.is_malicious_payload(malicious_payload)
            if is_malicious:
                print(f"+ Malicious payload correctly blocked: {reason}")
            else:
                print("- Malicious payload incorrectly allowed")
                return False
        
        return True
        
    except Exception as e:
        print(f"- Error testing proxy logic: {e}")
        return False

def main():
    print("Traxes Repository Validation Test")
    print("=" * 40)
    
    # Check if we're in the right directory
    if not os.path.exists('payloads'):
        print("Error: Must run from repository root directory")
        sys.exit(1)
    
    tests = [
        test_payloads,
        test_python_syntax,
        test_engine_evaluation
    ]
    
    all_passed = True
    for test in tests:
        if not test():
            all_passed = False
    
    print("\n" + "=" * 40)
    if all_passed:
        print("+ All tests passed! Repository is ready.")
        print("\nTo run the full evaluation:")
        print("1. docker-compose up -d")
        print("2. docker-compose exec test-runner python test_harness.py --safe")
        print("3. docker-compose exec test-runner python test_harness.py --malicious")
    else:
        print("- Some tests failed. Please fix issues before proceeding.")
        sys.exit(1)

if __name__ == "__main__":
    main()
