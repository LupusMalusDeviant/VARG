// ── Varg Playground — Monaco Editor interop ──────────────────────────────────

(function () {

// ── Example presets ───────────────────────────────────────────────────────────

const EX_HELLO = `agent Greeting {
    public void Run() {
        unsafe {
            var rl = readline_new();
            print("=== Varg Greeting ===");

            var name = readline_read(rl, "What's your name? ");
            print($"Hello, {name}! Welcome to Varg.");

            var city = readline_read(rl, "Where are you from? ");
            print($"Nice to meet you, {name} from {city}!");

            var fav = readline_read(rl, "Favourite programming language? ");
            if fav == "Varg" {
                print("Excellent taste -- you clearly have good instincts.");
            } else {
                print($"{fav} is cool, but have you tried compiling to native with Varg?");
            }

            print("Thanks for trying the Varg Playground!");
        }
    }
}
`;

const EX_FIBONACCI = `agent Fibonacci {
    public void Run() {
        unsafe {
            var rl = readline_new();
            print("=== Fibonacci Calculator ===");

            var input = readline_read(rl, "Enter n (0-20): ");
            var n     = parse_int(input);
            var result = self.calc(n);
            print($"fib({n}) = {result}");

            print("Sequence:");
            var i = 0;
            while i <= n {
                var fi = self.calc(i);
                print($"  fib({i}) = {fi}");
                i = i + 1;
            }
        }
    }

    public int calc(int n) {
        if n <= 1 {
            return n;
        }
        return self.calc(n - 1) + self.calc(n - 2);
    }
}
`;

const EX_CALCULATOR = `agent Calculator {
    public void Run() {
        unsafe {
            var rl = readline_new();
            print("=== Varg Calculator ===");
            print("Enter two integers.");

            var a_str = readline_read(rl, "First number:  ");
            var b_str = readline_read(rl, "Second number: ");

            var a = parse_int(a_str);
            var b = parse_int(b_str);

            var sum  = a + b;
            var diff = a - b;
            var prod = a * b;
            var hi   = max(a, b);
            var lo   = min(a, b);

            print($"  {a} + {b} = {sum}");
            print($"  {a} - {b} = {diff}");
            print($"  {a} * {b} = {prod}");
            print($"  max({a}, {b}) = {hi}");
            print($"  min({a}, {b}) = {lo}");
        }
    }
}
`;

const EX_GUESS = `agent GuessGame {
    public void Run() {
        unsafe {
            var rl       = readline_new();
            var secret   = 42;
            var attempts = 0;
            var solved   = false;

            print("=== Number Guessing Game ===");
            print("I'm thinking of a number between 1 and 100.");
            print("Type a number to guess, or 'quit' to give up.");
            print("");

            while solved == false {
                var input = readline_read(rl, "Guess: ");

                if input == "quit" {
                    print($"Giving up? The number was {secret}.");
                    solved = true;
                } else {
                    var guess    = parse_int(input);
                    attempts     = attempts + 1;

                    if guess == secret {
                        print($"Correct! You got it in {attempts} attempt(s).");
                        solved = true;
                    } else {
                        if guess < secret {
                            print("  Too low -- try higher.");
                        } else {
                            print("  Too high -- try lower.");
                        }
                    }
                }
            }
        }
    }
}
`;

const EX_AGENT = `agent AiAgent {
    public void Run() {
        unsafe {
            var rl = readline_new();
            print("=== Varg AI Agent ===");
            print("A minimal LLM chatbot powered by GPT-4o-mini.");
            print("");

            var provider = "openai";
            var model    = "gpt-4o-mini";

            var key = readline_read(rl, "OpenAI API Key (sk-...): ");
            if key == "" {
                print("No key entered. Exiting.");
                return;
            }
            set_env("OPENAI_API_KEY", key);

            print("Ready! Type your message, or 'quit' to exit.");
            print("");

            var running = true;
            while running == true {
                var input = readline_read(rl, "You: ");
                if input == "quit" {
                    print("Goodbye!");
                    running = false;
                } else {
                    var msgs  = [{"role": "user", "content": input}];
                    var reply = llm_chat(provider, model, msgs);
                    print($"AI: {reply}");
                    print("");
                }
            }
        }
    }
}
`;

const EX_OCAP = `agent SecureApp {
    public string readFile(string path, FileAccess fa) {
        return fs_read(path) or "(empty)";
    }

    public void writeFile(string path, string data, FileAccess fa) {
        fs_write(path, data);
    }

    public void Run() {
        unsafe {
            // Tokens can ONLY be constructed in unsafe{}
            var fa  = FileAccess {};
            var sys = SystemAccess {};
            var rl  = readline_new();

            print("=== OCAP Security Demo ===");
            print("FileAccess + SystemAccess required.");
            print("Without the tokens, builtins are blocked at compile time.");
            print("");

            var path    = readline_read(rl, "Filename: ");
            var content = readline_read(rl, "Content:  ");

            self.writeFile(path, content, fa);
            print("Written.");

            var back = self.readFile(path, fa);
            print($"Read back: {back}");
        }
    }
}
`;

const EX_HTTP = `agent WeatherApp {
    public void Run() {
        unsafe {
            var net = NetworkAccess {};
            var sys = SystemAccess {};
            var rl  = readline_new();

            print("=== Varg HTTP Fetcher ===");
            print("Live weather via wttr.in (no API key needed)");
            print("");

            var city = readline_read(rl, "City (e.g. Berlin): ");
            var url  = $"https://wttr.in/{city}?format=3";

            print($"Fetching {url} ...");
            var resp = fetch(url);
            print("");
            print(resp);
        }
    }
}
`;

const EX_SQLITE = `agent Notes {
    public void Run() {
        unsafe {
            var db  = db_open("notes.db");
            var sys = SystemAccess {};
            var rl  = readline_new();

            db_execute(db, "CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL)", []);

            print("=== Varg Notes (SQLite) ===");
            print("Commands:  add <text>  |  list  |  quit");
            print("Data is persisted in notes.db next to the binary.");
            print("");

            var running = true;
            while running == true {
                var input = readline_read(rl, "> ");

                if input.starts_with("add ") {
                    var text = input.substring(4, len(input));
                    db_execute(db, "INSERT INTO notes (text) VALUES (?1)", [text]);
                    print("Saved.");
                } else if input == "list" {
                    var rows = db_query(db, "SELECT id, text FROM notes ORDER BY id", []);
                    foreach row in rows {
                        print(row);
                    }
                } else if input == "quit" {
                    print("Bye!");
                    running = false;
                } else {
                    print("Unknown command. Try: add <text>, list, quit");
                }
            }
        }
    }
}
`;

const EXAMPLES = {
    hello:       { files: [{ name: 'main.varg', code: EX_HELLO }] },
    fibonacci:   { files: [{ name: 'main.varg', code: EX_FIBONACCI }] },
    calculator:  { files: [{ name: 'main.varg', code: EX_CALCULATOR }] },
    guess:       { files: [{ name: 'main.varg', code: EX_GUESS }] },
    agent:       { files: [{ name: 'main.varg', code: EX_AGENT }] },
    ocap:        { files: [{ name: 'main.varg', code: EX_OCAP }] },
    http:        { files: [{ name: 'main.varg', code: EX_HTTP }] },
    sqlite:      { files: [{ name: 'main.varg', code: EX_SQLITE }] },
};

// ── Monarch grammar ───────────────────────────────────────────────────────────
const MONARCH = {
    keywords: [
        'if','else','while','for','foreach','in','try','catch','throw','return',
        'from','where','select','orderby','descending','import','match','retry',
        'fallback','break','continue','unsafe','spawn','send','request','crate'
    ],
    declKeywords: [
        'public','private','system','async','agent','contract','struct','enum',
        'var','let','const','type','fn','prompt','self','new','override',
        'implements','pub','print','on_start','on_stop','on_message'
    ],
    typeKeywords: [
        'int','float','string','bool','void','ulong',
        'Tensor','Prompt','Context','Embedding',
        'FileAccess','NetworkAccess','DbAccess','LlmAccess','SystemAccess',
        'Result','Error','List','map','set',
        'GraphHandle','VectorStoreHandle','MemoryHandle','TracerHandle',
        'EventBusHandle','PipelineHandle','OrchestratorHandle','SelfImproverHandle',
        'BudgetHandle','CheckpointHandle','ChannelHandle','WorkflowHandle',
        'RegistryHandle','RateLimiterHandle','ImageHandle','AudioHandle',
        'SseHandle','DbHandle','WsHandle','McpHandle','McpServerHandle',
        'ReadlineHandle','ProcHandle'
    ],
    builtins: [
        'fs_read','fs_write','fs_append','fs_read_lines','fs_read_dir',
        'fs_read_bytes','fs_write_bytes','fs_append_bytes','fs_size',
        'create_dir','delete_file','path_exists','path_join','path_parent',
        'path_extension','path_stem',
        'fetch','http_request','http_serve','http_route','http_listen',
        'http_response','http_sse_route','sse_connect','sse_read','sse_close','sse_event',
        'db_open','db_execute','db_query',
        'ws_connect','ws_send','ws_receive','ws_close',
        'mcp_connect','mcp_list_tools','mcp_call_tool','mcp_disconnect',
        'mcp_server_new','mcp_server_register','mcp_server_run',
        'llm_chat','llm_complete','llm_structured','llm_stream','llm_embed_batch','llm_vision',
        'json_parse','json_get','json_get_int','json_get_bool','json_get_array','json_stringify',
        'graph_open','graph_add_node','graph_add_edge','graph_query','graph_traverse','graph_neighbors',
        'embed','vector_store_open','vector_store_upsert','vector_store_search',
        'vector_store_delete','vector_store_count','cosine_sim',
        'vector_build_index','vector_search_fast',
        'memory_open','memory_set','memory_get','memory_store','memory_recall',
        'memory_add_fact','memory_query_facts','memory_clear_working','memory_episode_count',
        'trace_start','trace_span','trace_end','trace_error','trace_event',
        'trace_set_attr','trace_export','trace_span_count',
        'event_bus_new','event_on','event_emit','event_count',
        'pipeline_new','pipeline_add_step','pipeline_run','pipeline_step_count',
        'orchestrator_new','orchestrator_add_task','orchestrator_run_all',
        'orchestrator_results','orchestrator_task_count','orchestrator_completed_count',
        'fan_out','fan_in',
        'self_improver_new','self_improver_record_success','self_improver_record_failure',
        'self_improver_recall','self_improver_success_rate','self_improver_iterations','self_improver_stats',
        'await_approval','await_input','await_choice',
        'ratelimiter_new','ratelimiter_acquire','ratelimiter_try_acquire',
        'rate_limit_acquire','rate_limit_try','rate_limit_reset',
        'budget_new','budget_track','budget_check','budget_remaining_tokens',
        'budget_remaining_usd_cents','budget_report','estimate_tokens',
        'checkpoint_open','checkpoint_save','checkpoint_load','checkpoint_clear',
        'checkpoint_exists','checkpoint_age',
        'channel_new','channel_send','channel_recv','channel_try_recv',
        'channel_recv_timeout','channel_len','channel_close','channel_is_closed',
        'prop_gen_int','prop_gen_float','prop_gen_bool','prop_gen_string',
        'prop_gen_int_list','prop_gen_string_list','prop_check','prop_assert',
        'image_load','image_from_base64','image_to_base64','image_format','image_size_bytes',
        'audio_load','audio_to_base64','audio_format','audio_size_bytes',
        'workflow_new','workflow_add_step','workflow_set_output','workflow_set_failed',
        'workflow_ready_steps','workflow_is_complete','workflow_get_output',
        'workflow_step_count','workflow_status',
        'registry_open','registry_install','registry_uninstall','registry_is_installed',
        'registry_version','registry_list','registry_search',
        'proc_spawn','proc_wait','proc_kill','proc_status',
        'home_dir','config_dir','data_dir','cache_dir','config_load_cascade',
        'readline_new','readline_read','readline_add_history','readline_load_history','readline_save_history',
        'base64_encode','base64_decode','base64_encode_file','http_download_base64',
        'pdf_create','pdf_add_section','pdf_add_text','pdf_save','pdf_to_base64',
        'exec','exec_status','env','sleep','timestamp',
        'time_millis','time_format','time_parse','time_add','time_diff',
        'encrypt','decrypt',
        'log_debug','log_info','log_warn','log_error',
        'abs','sqrt','floor','ceil','round','min','max','pow','parse_int','parse_float',
        'assert','assert_eq','assert_ne','assert_true','assert_false','assert_contains','assert_throws',
        'regex_match','regex_find_all','regex_replace',
        'len','push','pop','remove','contains','sort','filter','find','any','all',
        'keys','values','split','join','trim','to_upper','to_lower',
        'starts_with','ends_with','replace','substring','index_of',
        'pad_left','pad_right','char_at','chars','reverse','repeat'
    ],
    constants: ['true','false','null'],

    tokenizer: {
        root: [
            [/@\[[a-zA-Z_]\w*(?:\([^)]*\))?\]/, 'annotation'],
            [/"""/, { token: 'string.triple', next: '@tripleString' }],
            [/\$"/, { token: 'string.interp.delim', next: '@interpString' }],
            [/"/, { token: 'string.delim', next: '@string' }],
            [/\/\/.*$/, 'comment'],
            [/\/\*/, { token: 'comment', next: '@blockComment' }],
            [/\b\d+\.\d+\b/, 'number.float'],
            [/\b\d+\b/, 'number'],
            [/[a-zA-Z_]\w*/, {
                cases: {
                    '@keywords':     'keyword.control',
                    '@declKeywords': 'keyword.other',
                    '@typeKeywords': 'type',
                    '@builtins':     'support.function',
                    '@constants':    'constant.language',
                    '@default':      'identifier'
                }
            }],
            [/[{}()\[\]]/, '@brackets'],
            [/[=><!~?:&|+\-*\/^%@]+/, 'operator'],
            [/[;,.]/, 'delimiter'],
        ],
        tripleString: [
            [/"""/, { token: 'string.triple', next: '@pop' }],
            [/\{[^}]*\}/, 'string.interp.expr'],
            [/.|\n/, 'string.triple'],
        ],
        interpString: [
            [/"/, { token: 'string.interp.delim', next: '@pop' }],
            [/\{[^}]*\}/, 'string.interp.expr'],
            [/\\./, 'string.escape'],
            [/./, 'string.interp'],
        ],
        string: [
            [/"/, { token: 'string.delim', next: '@pop' }],
            [/\\./, 'string.escape'],
            [/./, 'string'],
        ],
        blockComment: [
            [/\*\//, { token: 'comment', next: '@pop' }],
            [/.|\n/, 'comment'],
        ],
    }
};

// ── VS Code dark+ theme ───────────────────────────────────────────────────────
const VARG_THEME = {
    base: 'vs-dark', inherit: true,
    rules: [
        { token: 'comment',             foreground: '6a9955', fontStyle: 'italic' },
        { token: 'keyword.control',     foreground: 'c586c0' },
        { token: 'keyword.other',       foreground: '569cd6' },
        { token: 'type',                foreground: '4ec9b0' },
        { token: 'support.function',    foreground: 'dcdcaa' },
        { token: 'constant.language',   foreground: '569cd6' },
        { token: 'number',              foreground: 'b5cea8' },
        { token: 'number.float',        foreground: 'b5cea8' },
        { token: 'string',              foreground: 'ce9178' },
        { token: 'string.triple',       foreground: 'ce9178' },
        { token: 'string.interp',       foreground: 'ce9178' },
        { token: 'string.delim',        foreground: 'ce9178' },
        { token: 'string.interp.delim', foreground: 'ce9178' },
        { token: 'string.interp.expr',  foreground: '9cdcfe' },
        { token: 'string.escape',       foreground: 'd7ba7d' },
        { token: 'annotation',          foreground: 'dcdcaa', fontStyle: 'bold' },
        { token: 'identifier',          foreground: '9cdcfe' },
        { token: 'operator',            foreground: 'd4d4d4' },
        { token: 'delimiter',           foreground: 'd4d4d4' },
    ],
    colors: {
        'editor.background':                    '#0c0e22',
        'editor.foreground':                    '#cdd6f4',
        'editorLineNumber.foreground':          '#3a4080',
        'editorActiveLineNumber.foreground':    '#6070c0',
        'editor.lineHighlightBackground':       '#12163a',
        'editorCursor.foreground':              '#6090ff',
        'editor.selectionBackground':           '#1e3060',
        'editorIndentGuide.background1':        '#1a1e50',
        'editorIndentGuide.activeBackground1':  '#2a3070',
    }
};

// ── Public API ────────────────────────────────────────────────────────────────
window.vargIde = {
    _editor:     null,
    _models:     {},          // filename → ITextModel
    _activeFile: 'main.varg',

    // ── init ─────────────────────────────────────────────────────────────────

    init: function (editorEl, resizeEl) {
        const ide = this;
        require.config({ paths: { vs: 'https://cdn.jsdelivr.net/npm/monaco-editor@0.47.0/min/vs' } });

        require(['vs/editor/editor.main'], function () {
            monaco.languages.register({ id: 'varg', extensions: ['.varg'], aliases: ['Varg'] });
            monaco.languages.setMonarchTokensProvider('varg', MONARCH);
            monaco.editor.defineTheme('varg-dark', VARG_THEME);

            // Initial model
            const mainModel = monaco.editor.createModel(EX_HELLO, 'varg');
            ide._models['main.varg'] = mainModel;
            ide._activeFile = 'main.varg';

            ide._editor = monaco.editor.create(editorEl, {
                model:                  mainModel,
                theme:                  'varg-dark',
                fontSize:               14,
                fontFamily:             '"Cascadia Code", "Fira Code", Consolas, monospace',
                fontLigatures:          true,
                lineHeight:             22,
                minimap:                { enabled: true },
                scrollBeyondLastLine:   false,
                automaticLayout:        true,
                renderLineHighlight:    'all',
                bracketPairColorization:{ enabled: true },
                smoothScrolling:        true,
                cursorBlinking:         'phase',
                padding:                { top: 12, bottom: 12 },
                suggest:                { showKeywords: true },
            });
        });

        // Drag-to-resize output panel
        if (resizeEl) {
            let dragStart = 0, panelStartH = 0;
            const shell = document.getElementById('shell');
            const panel = document.getElementById('panel');

            resizeEl.addEventListener('mousedown', function (e) {
                dragStart   = e.clientY;
                panelStartH = panel.offsetHeight;
                document.body.style.cursor     = 'row-resize';
                document.body.style.userSelect = 'none';

                function onMove(e) {
                    const delta = dragStart - e.clientY;
                    const newH  = Math.max(80, Math.min(window.innerHeight * 0.75, panelStartH + delta));
                    const rows  = shell.style.gridTemplateRows.split(' ');
                    rows[3]     = newH + 'px';
                    shell.style.gridTemplateRows = rows.join(' ');
                }
                function onUp() {
                    document.body.style.cursor     = '';
                    document.body.style.userSelect = '';
                    window.removeEventListener('mousemove', onMove);
                    window.removeEventListener('mouseup',   onUp);
                    if (ide._editor) ide._editor.layout();
                }
                window.addEventListener('mousemove', onMove);
                window.addEventListener('mouseup',   onUp);
                e.preventDefault();
            });
        }
    },

    // ── multi-file API ────────────────────────────────────────────────────────

    getFiles: function () {
        return Object.fromEntries(
            Object.entries(this._models).map(([k, v]) => [k, v.getValue()])
        );
    },

    activeFile: function () {
        return this._activeFile;
    },

    addFile: function (name, code) {
        if (this._models[name]) { this.switchFile(name); return; }
        this._models[name] = monaco.editor.createModel(code || '', 'varg');
        this.switchFile(name);
    },

    switchFile: function (name) {
        if (!this._models[name] || !this._editor) return;
        this._activeFile = name;
        this._editor.setModel(this._models[name]);
        this._editor.focus();
    },

    removeFile: function (name) {
        if (Object.keys(this._models).length <= 1) return;
        const m = this._models[name];
        if (!m) return;
        delete this._models[name];
        m.dispose();
        // Switch to first remaining
        const firstKey = Object.keys(this._models)[0];
        this._activeFile = firstKey;
        if (this._editor) this._editor.setModel(this._models[firstKey]);
    },

    loadExample: function (name) {
        const ex = EXAMPLES[name];
        if (!ex || !this._editor) return [];

        // Dispose old models
        for (const m of Object.values(this._models)) m.dispose();
        this._models = {};

        // Create new models
        for (const f of ex.files)
            this._models[f.name] = monaco.editor.createModel(f.code, 'varg');

        this._activeFile = ex.files[0].name;
        this._editor.setModel(this._models[this._activeFile]);
        this._editor.focus();

        return ex.files.map(f => f.name);
    },

    // ── download ──────────────────────────────────────────────────────────────

    downloadBytes: function (base64, filename) {
        const bytes = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
        const blob  = new Blob([bytes], { type: 'application/octet-stream' });
        const url   = URL.createObjectURL(blob);
        const a     = document.createElement('a');
        a.href      = url;
        a.download  = filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        setTimeout(() => URL.revokeObjectURL(url), 1000);
    },
};

})();
