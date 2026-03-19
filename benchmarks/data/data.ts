const start = performance.now();

// Generate 100k numbers
const numbers: number[] = Array.from({ length: 100000 }, (_, i) => i);

// Filter + map
const doubled = numbers.filter(n => n % 2 === 0).map(n => n * 2);

// Sum
const total = doubled.reduce((a, b) => a + b, 0);

// Word frequency
const words = ["rust", "varg", "ai", "agent", "compile", "type", "safe", "fast", "native", "async"];
const freq: Record<string, number> = {};
for (let j = 0; j < 10000; j++) {
    const word = words[j % words.length];
    freq[word] = (freq[word] || 0) + 1;
}

const elapsed = performance.now() - start;
console.log(`Sum: ${total}`);
console.log(`Freq entries: ${Object.keys(freq).length}`);
console.log(`Time: ${Math.round(elapsed)}ms`);
