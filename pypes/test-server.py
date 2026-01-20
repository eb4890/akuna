#!/usr/bin/env python3
"""
Simple HTTP server for testing the Pypes Registry POC.
Serves components from test-registry/ directory.
"""
import http.server
import socketserver
import os

PORT = 8080
DIRECTORY = "test-registry"

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIRECTORY, **kwargs)
    
    def end_headers(self):
        # Add CORS headers for local testing
        self.send_header('Access-Control-Allow-Origin', '*')
        super().end_headers()

if __name__ == "__main__":
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    
    print(f"Starting test registry server on http://localhost:{PORT}")
    print(f"Serving: {os.path.abspath(DIRECTORY)}")
    print("\nAvailable component:")
    print("  remote://localhost:8080/test-skill@1.0.0")
    print("\nPress Ctrl+C to stop\n")
    
    with socketserver.TCPServer(("", PORT), Handler) as httpd:
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down server...")
