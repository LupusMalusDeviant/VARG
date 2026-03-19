import time, json
start = time.time()
items = []
for i in range(1000):
    items.append({"id":i,"name":f"item_{i}","value":i*17,"active":i%3!=0})
json_str = json.dumps(items)
parsed = json.loads(json_str)
output = json.dumps(parsed)
elapsed = (time.time() - start) * 1000
print(f"JSON length: {len(output)}")
print(f"Time: {elapsed:.0f}ms")
