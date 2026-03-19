import sys
for line in sys.stdin:
    if "Claude" not in line and "claude" not in line.lower():
        sys.stdout.write(line)
