import json

def clean_data():
    with open('varg_trainingsdaten.jsonl', 'r', encoding='utf-8') as f:
        lines = f.readlines()

    # The original data was the first 276 lines
    original_lines = lines[:276]
    
    clean_lines = []
    for line in original_lines:
        lower = line.lower()
        if 'fallback' in lower or 'agent t_' in lower:
            continue
        try:
            # Validate JSON
            j = json.loads(line)
            clean_lines.append(j)
        except:
            pass
            
    print(f"Original clean examples: {len(clean_lines)}")
    
    # Let's add exactly ONE high-quality, unique example for each of our advanced features
    advanced_examples = [
        {
            "instruction": "Zeige, wie du den Varg Vector Store nutzt, um Dokumente zu indizieren und basierend auf Ähnlichkeit abzurufen.",
            "output": """agent SemanticSearch {
    public void Run() {
        var store = vector_store_open("doc_store");
        var meta = {"type": "article", "author": "admin"};
        
        var embedding = embed("Autonome KI-Agenten werden die Welt verändern.");
        vector_store_upsert(store, "doc_1", embedding, meta);
        
        var query = embed("Zukunft der KI");
        var results = vector_store_search(store, query, 1);
        
        if (results.len() > 0) {
            print $"Relevantestes Dokument hat ID: {results[0]["id"]}";
        }
    }
}"""
        },
        {
            "instruction": "Implementiere ein semantisches Gedächtnis (Memory), das Vorlieben eines Nutzers langfristig speichert und wieder abruft.",
            "output": """agent PersonalAssistant {
    public void Run() {
        var mem = memory_open("UserPrefsBot");
        memory_set(mem, "current_user", "Alice");
        
        // Speichere eine spezifische Episode
        memory_store(mem, "Nutzer fragte nach veganen Rezepten", {"intent": "food"});
        
        // Füge einen generellen Fakt in den Knowledge Graph ein
        memory_add_fact(mem, "DietaryPreference", {"diet": "vegan"});
        
        var recalls = memory_recall(mem, "Essen", 1);
        print $"Erinnerung: {recalls[0]["content"]}";
    }
}"""
        },
        {
            "instruction": "Schreibe einen Agenten, der einen eingebetteten Knowledge Graph erstellt, Knoten hinzufügt und Relationen abfragt.",
            "output": """agent GraphBuilder {
    public void Run() {
        var graph = graph_open("KnowledgeBase");
        var alice = graph_add_node(graph, "Person", {"name": "Alice"});
        var bob = graph_add_node(graph, "Person", {"name": "Bob"});
        
        graph_add_edge(graph, alice, "knows", bob, {"since": "2024"});
        
        var people = graph_query(graph, "Person");
        print $"Graph enthält {people.len()} Personen.";
    }
}"""
        },
        {
            "instruction": "Wie erstellt man in Varg einen MCP-Server, der ein Tool zur Verfügung stellt?",
            "output": """agent ToolsServer {
    public void Run() {
        var srv = mcp_server_new("Calculator", "1.0");
        
        mcp_server_register(srv, "add", (args) => {
            var a = json_get_int(args, "/a");
            var b = json_get_int(args, "/b");
            return $"Result: {a + b}";
        });
        
        print "Starte MCP Server auf stdio...";
        mcp_server_run(srv);
    }
}"""
        },
        {
            "instruction": "Nutze die Self-Improving API, um Erfolge und Misserfolge eines Agenten zu loggen und die Erfolgsquote auszuwerten.",
            "output": """agent AdaptiveBot {
    public void Run() {
        var improver = self_improver_new("AdaptiveBot", 5);
        
        self_improver_record_success(improver, "Daten extrahiert", "Nutze Regex");
        self_improver_record_failure(improver, "API Timeout", "Brauche besseren Retry");
        
        var memory_flash = self_improver_recall(improver, "Fehler bei API", 1);
        var stats = self_improver_stats(improver);
        
        var rate = stats["success_rate"];
        print $"Derzeitige Erfolgsquote: {rate}";
    }
}"""
        }
    ]
    
    clean_lines.extend(advanced_examples)
    
    with open('varg_trainingsdaten.jsonl', 'w', encoding='utf-8') as f:
        for item in clean_lines:
            f.write(json.dumps(item, ensure_ascii=False) + '\n')
            
    print(f"Total pure gold examples: {len(clean_lines)}")

if __name__ == '__main__':
    clean_data()
