# Collections: build a list, filter, map, sum, then count words into a map.
# Every language here does the same work in the same order.
import time

start = time.time()

numbers = []
for i in range(100000):
    numbers.append(i)

doubled = [n * 2 for n in numbers if n % 2 == 0]
total = 0
for n in doubled:
    total += n

words = ["rust", "varg", "ai", "agent", "compile", "type", "safe", "fast", "native", "async"]
freq = {"seed": 0}
for j in range(10000):
    word = words[j % 10]
    freq[word] = freq.get(word, 0) + 1

elapsed = (time.time() - start) * 1000
print(f"Sum: {total}")
print(f"Freq entries: {len(freq)}")
print(f"Time: {elapsed:.0f}ms")
