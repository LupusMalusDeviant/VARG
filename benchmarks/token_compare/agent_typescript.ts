import express from "express";

const app = express();
app.use(express.json());

app.get("/health", (req, res) => {
    res.json({ status: "ok" });
});

app.post("/ask", async (req, res) => {
    const question = req.body.question;
    const response = await fetch(`https://api.example.com/ask?q=${question}`);
    const answer = await response.text();
    res.send(answer);
});

app.listen(8080, "0.0.0.0");
