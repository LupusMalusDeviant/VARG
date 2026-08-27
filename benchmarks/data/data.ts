// Collections: build a list, filter, map, sum, then count words into a map.
// Every language here does the same work in the same order.
const start = performance.now();

const numbers: number[] = [];
for (let i = 0; i < 100000; i++) numbers.push(i);

const doubled = numbers.filter((n) => n % 2 === 0).map((n) => n * 2);
let total = 0;
for (const n of doubled) total += n;

const words = ["rust", "varg", "ai", "agent", "compile", "type", "safe", "fast", "native", "async"];
const freq = new Map<string, number>([["seed", 0]]);
for (let j = 0; j < 10000; j++) {
    const word = words[j % 10];
    freq.set(word, (freq.get(word) ?? 0) + 1);
}

const elapsed = performance.now() - start;
console.log(`Sum: ${total}`);
console.log(`Freq entries: ${freq.size}`);
console.log(`Time: ${Math.round(elapsed)}ms`);
