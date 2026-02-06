#!/usr/bin/python3
import os

print("Content-Type: text/html")
print()
print("<html><body>")
print("<h1>Hello from Python CGI!</h1>")
print("<p>REQUEST_METHOD: {}</p>".format(os.environ.get("REQUEST_METHOD", "Unknown")))
print("<p>PATH_INFO: {}</p>".format(os.environ.get("PATH_INFO", "Unknown")))
print("</body></html>")
