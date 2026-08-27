import time
parts = ["item-" + str(i) for i in range(200000)]
text = " ".join(parts)

start = time.time()
counts = {"seed": 0}
for w in text.split(" "):
    counts[w] = counts.get(w, 0) + 1
keys = sorted(counts.keys())
elapsed = (time.time() - start) * 1000
print(f"distinct = {len(keys)}")
print(f"Time: {elapsed:.0f}ms")
