# Build one JSON document by string concatenation, parse it, serialise it again.
# Every language here does the same work on the same input. This file used to build native dicts
# while the C# and TypeScript versions also filtered the parsed result, so the three were not
# measuring the same thing.
import time, json

start = time.time()

items = "["
for i in range(1000):
    if i > 0:
        items += ","
    active = "false" if i % 3 == 0 else "true"
    items += '{"id":%d,"name":"item_%d","value":%d,"active":%s}' % (i, i, i * 17, active)
items += "]"

parsed = json.loads(items)
output = json.dumps(parsed, separators=(",", ":"))

elapsed = (time.time() - start) * 1000
print(f"JSON length: {len(output)}")
print(f"Time: {elapsed:.0f}ms")
