from flask import Flask, request, jsonify
import requests

app = Flask(__name__)

@app.route("/health", methods=["GET"])
def health():
    return jsonify({"status": "ok"})

@app.route("/ask", methods=["POST"])
def ask():
    body = request.get_json()
    question = body["question"]
    answer = requests.get(f"https://api.example.com/ask?q={question}")
    return answer.text

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=8080)
