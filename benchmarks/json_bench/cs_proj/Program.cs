// Build one JSON document by string concatenation, parse it, serialise it again.
// Every language here does the same work on the same input. This file used to deserialise into
// Dictionary<string, JsonElement> and filter the result as well, which the Varg version did not.
using System.Diagnostics;
using System.Text;
using System.Text.Json;

var sw = Stopwatch.StartNew();

var sb = new StringBuilder("[");
for (int i = 0; i < 1000; i++)
{
    if (i > 0) sb.Append(',');
    var active = i % 3 == 0 ? "false" : "true";
    sb.Append($"{{\"id\":{i},\"name\":\"item_{i}\",\"value\":{i * 17},\"active\":{active}}}");
}
sb.Append(']');
var items = sb.ToString();

var parsed = JsonSerializer.Deserialize<JsonElement>(items);
var output = JsonSerializer.Serialize(parsed);

sw.Stop();
Console.WriteLine($"JSON length: {output.Length}");
Console.WriteLine($"Time: {sw.ElapsedMilliseconds}ms");
