const parts: string[] = [];
for (let i = 0; i < 200000; i++) parts.push("item-" + i);
const text = parts.join(" ");

const start = Date.now();
const counts = new Map<string, number>([["seed", 0]]);
for (const w of text.split(" ")) counts.set(w, (counts.get(w) ?? 0) + 1);
const keys = [...counts.keys()].sort();
const elapsed = Date.now() - start;
console.log(`distinct = ${keys.length}`);
console.log(`Time: ${elapsed}ms`);
