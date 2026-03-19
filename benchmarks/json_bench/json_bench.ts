const start = performance.now();

// Build 1000 objects
const items = Array.from({ length: 1000 }, (_, i) => ({
    id: i,
    name: `item_${i}`,
    value: i * 17,
    active: i % 3 !== 0,
}));

// Serialize
const jsonStr = JSON.stringify(items);

// Parse back
const parsed = JSON.parse(jsonStr);

// Filter active
const active = parsed.filter((x: any) => x.active);

// Serialize filtered
const output = JSON.stringify(active);

const elapsed = performance.now() - start;
console.log(`JSON length: ${output.length}`);
console.log(`Active items: ${active.length}`);
console.log(`Time: ${Math.round(elapsed)}ms`);
