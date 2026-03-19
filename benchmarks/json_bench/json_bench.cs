using System.Diagnostics;
using System.Text.Json;

var sw = Stopwatch.StartNew();

// Build 1000 objects
var items = Enumerable.Range(0, 1000)
    .Select(i => new { id = i, name = $"item_{i}", value = i * 17, active = i % 3 != 0 })
    .ToList();

// Serialize
var jsonStr = JsonSerializer.Serialize(items);

// Parse back
var parsed = JsonSerializer.Deserialize<List<Dictionary<string, JsonElement>>>(jsonStr)!;

// Filter active
var active = parsed.Where(x => x["active"].GetBoolean()).ToList();

// Serialize filtered
var output = JsonSerializer.Serialize(active);

sw.Stop();
Console.WriteLine($"JSON length: {output.Length}");
Console.WriteLine($"Active items: {active.Count}");
Console.WriteLine($"Time: {sw.ElapsedMilliseconds}ms");
