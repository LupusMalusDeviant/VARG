import json
import random

domains = ['Finance', 'Weather', 'SmartHome', 'Crypto', 'Log', 'Math', 'DevOps', 'CRM', 'Health', 'IOT', 'Analytics', 'Storage']
verbs = ['Check', 'Process', 'Manage', 'Analyze', 'Export', 'Sync', 'Monitor', 'Calculate', 'Fetch', 'Store']

def get_name(domain):
    return random.choice(verbs) + domain + 'Agent'

patterns = [
    # 0: Basic math/logic
    {'instruction': 'Schreibe einen Agenten, der alle Zahlen von 1 bis {num} filtert und ausgibt.',
     'output': '''agent {name} {{
    public void Run() {{
        var nums = [];
        for i in 1..{num} {{
            if (i % 2 == 0) {{
                nums.push(i);
            }}
        }}
        print $"Gefiltert: {{nums.len()}} Zahlen";
    }}
}}'''},

    # 1: FileAccess
    {'instruction': 'Erstelle einen Agenten, der Sensordaten in eine Textdatei schreibt. Nutze OCAP.',
     'output': '''agent {name} {{
    public void SaveData(string data, FileAccess fs) {{
        fs_write("data_{domain}.txt", data)?;
    }}

    public void Run() {{
        unsafe {{
            var fs = FileAccess{{}};
            self.SaveData("{{\\"sensor\\": \\"{domain}\\", \\"value\\": {num} }}", fs);
        }}
    }}
}}'''},

    # 2: NetworkAccess
    {'instruction': 'Schreibe einen Agenten, der {domain}-Daten von einer API abruft.',
     'output': '''agent {name} {{
    public async string FetchApi(NetworkAccess net) {{
        var url = "https://api.example.com/{domain}/data";
        var resp = fetch(url, "GET")?;
        return resp;
    }}

    public void Run() {{
        unsafe {{
            var net = NetworkAccess{{}};
            var result = self.FetchApi(net) or "Fehler beim Abruf";
            print result;
        }}
    }}
}}'''},

    # 3: DbAccess
    {'instruction': 'Erstelle einen Agenten zur Verwaltung von {domain} in einer SQLite-Datenbank.',
     'output': '''agent {name} {{
    public void Run() {{
        unsafe {{
            var db_cap = DbAccess{{}};
            var db = db_open("{domain}.db");
            db_execute(db, "CREATE TABLE IF NOT EXISTS records (id INTEGER PRIMARY KEY, value TEXT)", []);
            db_execute(db, "INSERT INTO records (value) VALUES (?1)", ["{num}"]);
            var rows = db_query(db, "SELECT * FROM records", []);
            print $"Gefunden: {{rows.len()}} Eintraege";
        }}
    }}
}}'''},

    # 4: Vector Store
    {'instruction': 'Schreibe einen Varg Agenten, der embeddings für {domain}-Texte in einem Vector Store speichert.',
     'output': '''agent {name} {{
    public void Run() {{
        var store = vector_store_open("{domain}_store");
        var meta = {{"category": "{domain}"}};
        var embedding = embed("Dies ist ein Text über {domain}");
        vector_store_upsert(store, "doc_{num}", embedding, meta);
        
        var results = vector_store_search(store, embedding, 3);
        print $"Ähnliche Dokumente gefunden: {{results.len()}}";
    }}
}}'''},

    # 5: Graph Database
    {'instruction': 'Erstelle einen Knowledge Graph für {domain}-Objekte und verknüpfe diese.',
     'output': '''agent {name} {{
    public void Run() {{
        var g = graph_open("{domain}Graph");
        var node1 = graph_add_node(g, "Entity", {{"name": "ItemA", "type": "{domain}"}});
        var node2 = graph_add_node(g, "Entity", {{"name": "ItemB", "type": "Related"}});
        graph_add_edge(g, node1, "connects_to", node2, {{}});

        var entities = graph_query(g, "Entity");
        print $"Graph enthält {{entities.len()}} Knoten";
    }}
}}'''},

    # 7: Agent Memory
    {'instruction': 'Implementiere einen Agenten mit Episodic und Semantic Memory für {domain}-Konversationen.',
     'output': '''agent {name} {{
    public void Run() {{
        var mem = memory_open("{domain}Bot");
        
        memory_set(mem, "current_context", "{domain}");
        
        memory_store(mem, "Nutzer fragte nach {domain} Updates", {{"emotion": "neutral"}});
        var fact_id = memory_add_fact(mem, "UserPreference", {{"topic": "{domain}"}});
        
        var recalls = memory_recall(mem, "{domain} Updates", 2);
        print $"Erinnere mich an {{recalls.len()}} Ereignisse.";
    }}
}}'''},

    # 8: MCP Server
    {'instruction': 'Erstelle einen MCP Server Agenten, der {domain}-Tools exponiert.',
     'output': '''agent {name} {{
    public void Run() {{
        var server = mcp_server_new("{domain}Tools", "1.0.0");
        
        mcp_server_register(server, "get_info", (args) => {{
            return $"Info für {domain}: {{args}}";
        }});
        
        mcp_server_run(server);
    }}
}}'''},

    # 9: Self-Improving Mechanism
    {'instruction': 'Nutze die Self-Improving API um {domain}-Fehler zu lernen.',
     'output': '''agent {name} {{
    public void Run() {{
        var si = self_improver_new("{domain}Learner", 3);
        
        // Simuliere Lernen
        self_improver_record_success(si, "Parse {domain} XML", "Used Regex cleanly");
        self_improver_record_failure(si, "Connect Database", "Missing credentials");
        
        var stats = self_improver_stats(si);
        var sr = stats["success_rate"];
        print $"Erfolgsquote: {{sr}}";
    }}
}}'''}
]

generated_data = []
target_count = 224

random.seed(42)
for _ in range(target_count):
    pat = random.choice(patterns)
    domain_choice = random.choice(domains)
    name_choice = get_name(domain_choice)
    num_choice = random.randint(10, 9999)
    port_choice = random.randint(3000, 9000)
    
    inst = pat['instruction'].format(domain=domain_choice, name=name_choice, num=num_choice, port=port_choice)
    out = pat['output'].format(domain=domain_choice, name=name_choice, num=num_choice, port=port_choice)
    
    generated_data.append({'instruction': inst, 'output': out})

# truncate back to 276 lines first
with open('varg_trainingsdaten.jsonl', 'r', encoding='utf-8') as f:
    lines = f.readlines()
with open('varg_trainingsdaten.jsonl', 'w', encoding='utf-8') as f:
    f.writelines(lines[:276])

with open("varg_trainingsdaten.jsonl", "a", encoding="utf-8") as f:
    for item in generated_data:
        f.write(json.dumps(item, ensure_ascii=False) + "\n")

print(f"Added {target_count} items. Total should be ~500.")
