// Collections: build a list, filter, map, sum, then count words into a map.
// Every language here does the same work in the same order. This file used to reach the same
// answer by a different route, so the comparison was partly between two different programs.
using System.Diagnostics;

var sw = Stopwatch.StartNew();

var numbers = new List<long>(100000);
for (long i = 0; i < 100000; i++) numbers.Add(i);

var doubled = numbers.Where(n => n % 2 == 0).Select(n => n * 2).ToList();
long total = 0;
foreach (var n in doubled) total += n;

var words = new[] { "rust", "varg", "ai", "agent", "compile", "type", "safe", "fast", "native", "async" };
var freq = new Dictionary<string, long> { { "seed", 0 } };
for (int j = 0; j < 10000; j++)
{
    var word = words[j % 10];
    freq[word] = freq.TryGetValue(word, out var c) ? c + 1 : 1;
}

sw.Stop();
Console.WriteLine($"Sum: {total}");
Console.WriteLine($"Freq entries: {freq.Count}");
Console.WriteLine($"Time: {sw.ElapsedMilliseconds}ms");
