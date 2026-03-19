var builder = WebApplication.CreateBuilder(args);
var app = builder.Build();

app.MapGet("/health", () => Results.Json(new { status = "ok" }));

app.MapPost("/ask", async (AskRequest body) => {
    var client = new HttpClient();
    var answer = await client.GetStringAsync($"https://api.example.com/ask?q={body.Question}");
    return answer;
});

app.Run("http://0.0.0.0:8080");

record AskRequest(string Question);
