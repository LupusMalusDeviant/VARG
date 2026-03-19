import time
start = time.time()
total = 0
count = 0
for i in range(100000):
    if i % 2 == 0:
        total += i * 2
        count += 1
words = ["rust","varg","ai","agent","compile","type","safe","fast","native","async"]
freq = {w:0 for w in words}
for j in range(10000):
    word = words[j % 10]
    freq[word] = freq.get(word, 0) + 1
elapsed = (time.time() - start) * 1000
print(f"Sum: {total}")
print(f"Freq entries: {len(freq.keys())}")
print(f"Time: {elapsed:.0f}ms")
