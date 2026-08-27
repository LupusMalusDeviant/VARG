using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;

class WordFreq {
    static void Main() {
        var parts = new List<string>(200000);
        for (int i = 0; i < 200000; i++) parts.Add("item-" + i.ToString());
        var text = string.Join(" ", parts);

        var sw = Stopwatch.StartNew();
        var counts = new Dictionary<string, long> { { "seed", 0 } };
        foreach (var w in text.Split(' '))
            counts[w] = counts.TryGetValue(w, out var c) ? c + 1 : 1;
        var keys = counts.Keys.OrderBy(k => k, StringComparer.Ordinal).ToList();
        sw.Stop();
        Console.WriteLine($"distinct = {keys.Count}");
        Console.WriteLine($"Time: {sw.ElapsedMilliseconds}ms");
    }
}
