import json

new_examples = [
    {
        "instruction": "Zeige, wie du die neuen flexiblen Methoden-Modifikatoren verwendest, wie zum Beispiel async public void anstatt public async void.",
        "output": "agent AsyncWorker {\n    async public void Process() {\n        var data = \"Daten geladen\";\n        print data;\n    }\n}"
    },
    {
        "instruction": "Schreibe einen Agenten, der eine String-Interpolation nutzt, die intern escapte Anführungszeichen erfolgreich handhabt.",
        "output": "agent StringFormatter {\n    public void Run() {\n        var wert = 42;\n        // Sichere Interpolation ohne Syntaxfehler durch Quotes\n        var msg = $\"Der Wert ist {wert} und er ist \\\"fertig\\\".\";\n        print msg;\n    }\n}"
    },
    {
        "instruction": "Zeige wie du das explizite __varg_-Präfix bei Methodenaufrufen, wie etwa beim Vector Store, weglassen kannst.",
        "output": "agent StoreDemo {\n    public void Run() {\n        var store = __varg_vector_store_open(\"docs\");\n        var query = embed(\"test\");\n        // Der Compiler entfernt bei Methodenaufrufen automatisch Präfixe vom Namen.\n        var count = store.vector_store_count(store);\n        // Die results Variable enthält die gefunden Dokumente.\n        print $\"Das Store hat {count} Dokumente.\";\n    }\n}"
    }
]

with open("varg_trainingsdaten.jsonl", "a", encoding="utf-8") as f:
    for ex in new_examples:
        f.write(json.dumps(ex, ensure_ascii=False) + "\n")
