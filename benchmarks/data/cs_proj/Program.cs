using System.Diagnostics;

var sw = Stopwatch.StartNew();

// Generate 100k numbers
var numbers = Enumerable.Range(0, 100000).ToList();

// Filter + map
var doubled = numbers.Where(n => n % 2 == 0).Select(n => n * 2).ToList();

// Sum
var total = doubled.Sum(n => (long)n);

// Word frequency
var words = new[] { "rust", "varg", "ai", "agent", "compile", "type", "safe", "fast", "native", "async" };
var freq = new Dictionary<string, int>();
for (int j = 0; j < 10000; j++)
{
    var word = words[j % words.Length];
    if (freq.ContainsKey(word))
        freq[word]++;
    else
        freq[word] = 1;
}

sw.Stop();
Console.WriteLine($"Sum: {total}");
Console.WriteLine($"Freq entries: {freq.Count}");
Console.WriteLine($"Time: {sw.ElapsedMilliseconds}ms");
