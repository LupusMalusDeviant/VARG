// Build one JSON document by string concatenation, parse it, serialise it again.
// Every language here does the same work on the same input.
const start = performance.now();

let items = "[";
for (let i = 0; i < 1000; i++) {
    if (i > 0) items += ",";
    const active = i % 3 === 0 ? "false" : "true";
    items += `{"id":${i},"name":"item_${i}","value":${i * 17},"active":${active}}`;
}
items += "]";

const parsed = JSON.parse(items);
const output = JSON.stringify(parsed);

const elapsed = performance.now() - start;
console.log(`JSON length: ${output.length}`);
console.log(`Time: ${Math.round(elapsed)}ms`);
