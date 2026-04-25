// ── Varg Playground — Monaco Editor interop ──────────────────────────────────

(function () {

// ── Default starter code ──────────────────────────────────────────────────────
const DEFAULT_CODE = `agent Hello {
    public void Run() {
        // String interpolation
        var name = "World";
        print($"Hello, {name}!");

        // Iterator chain — map / filter / any
        var nums = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        var evens   = nums.filter((x) => x % 2 == 0);
        var doubled = evens.map((x) => x * 2);
        print($"Evens doubled: {doubled}");
        print($"Any > 15? {any(doubled, (x) => x > 15)}");

        // Closures + variables
        var threshold = 5;
        var big = nums.filter((x) => x > threshold);
        print($"Numbers above {threshold}: {big}");

        // String operations
        var greeting = "Hello, Varg!";
        print($"Upper: {to_upper(greeting)}");
        print($"Length: {len(greeting)}");

        // Pattern matching (as statement)
        var score = 72;
        match score {
            90..100 => print("Grade: A"),
            70..89  => print("Grade: B"),
            50..69  => print("Grade: C"),
            _       => print("Grade: F"),
        }
    }
}
`;

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
        // File I/O
        'fs_read','fs_write','fs_append','fs_read_lines','fs_read_dir',
        'fs_read_bytes','fs_write_bytes','fs_append_bytes','fs_size',
        'create_dir','delete_file','path_exists','path_join','path_parent',
        'path_extension','path_stem',
        // HTTP
        'fetch','http_request','http_serve','http_route','http_listen',
        'http_response','http_sse_route','sse_connect','sse_read','sse_close','sse_event',
        // Database
        'db_open','db_execute','db_query',
        // WebSocket
        'ws_connect','ws_send','ws_receive','ws_close',
        // MCP
        'mcp_connect','mcp_list_tools','mcp_call_tool','mcp_disconnect',
        'mcp_server_new','mcp_server_register','mcp_server_run',
        // LLM
        'llm_chat','llm_complete','llm_structured','llm_stream','llm_embed_batch','llm_vision',
        // JSON
        'json_parse','json_get','json_get_int','json_get_bool','json_get_array','json_stringify',
        // Graph
        'graph_open','graph_add_node','graph_add_edge','graph_query','graph_traverse','graph_neighbors',
        // Vector
        'embed','vector_store_open','vector_store_upsert','vector_store_search',
        'vector_store_delete','vector_store_count','cosine_sim',
        'vector_build_index','vector_search_fast',
        // Memory
        'memory_open','memory_set','memory_get','memory_store','memory_recall',
        'memory_add_fact','memory_query_facts','memory_clear_working','memory_episode_count',
        // Tracing
        'trace_start','trace_span','trace_end','trace_error','trace_event',
        'trace_set_attr','trace_export','trace_span_count',
        // Pipeline / Events
        'event_bus_new','event_on','event_emit','event_count',
        'pipeline_new','pipeline_add_step','pipeline_run','pipeline_step_count',
        // Orchestration
        'orchestrator_new','orchestrator_add_task','orchestrator_run_all',
        'orchestrator_results','orchestrator_task_count','orchestrator_completed_count',
        'fan_out','fan_in',
        // Self-improve
        'self_improver_new','self_improver_record_success','self_improver_record_failure',
        'self_improver_recall','self_improver_success_rate','self_improver_iterations','self_improver_stats',
        // HITL
        'await_approval','await_input','await_choice',
        // Rate limiting
        'ratelimiter_new','ratelimiter_acquire','ratelimiter_try_acquire',
        'rate_limit_acquire','rate_limit_try','rate_limit_reset',
        // Budget
        'budget_new','budget_track','budget_check','budget_remaining_tokens',
        'budget_remaining_usd_cents','budget_report','estimate_tokens',
        // Checkpoint
        'checkpoint_open','checkpoint_save','checkpoint_load','checkpoint_clear',
        'checkpoint_exists','checkpoint_age',
        // Channels
        'channel_new','channel_send','channel_recv','channel_try_recv',
        'channel_recv_timeout','channel_len','channel_close','channel_is_closed',
        // Property testing
        'prop_gen_int','prop_gen_float','prop_gen_bool','prop_gen_string',
        'prop_gen_int_list','prop_gen_string_list','prop_check','prop_assert',
        // Multimodal
        'image_load','image_from_base64','image_to_base64','image_format','image_size_bytes',
        'audio_load','audio_to_base64','audio_format','audio_size_bytes',
        // Workflow
        'workflow_new','workflow_add_step','workflow_set_output','workflow_set_failed',
        'workflow_ready_steps','workflow_is_complete','workflow_get_output',
        'workflow_step_count','workflow_status',
        // Registry
        'registry_open','registry_install','registry_uninstall','registry_is_installed',
        'registry_version','registry_list','registry_search',
        // Process
        'proc_spawn','proc_wait','proc_kill','proc_status',
        // Dirs
        'home_dir','config_dir','data_dir','cache_dir','config_load_cascade',
        // Readline
        'readline_new','readline_read','readline_add_history','readline_load_history','readline_save_history',
        // Base64
        'base64_encode','base64_decode','base64_encode_file','http_download_base64',
        // PDF
        'pdf_create','pdf_add_section','pdf_add_text','pdf_save','pdf_to_base64',
        // System
        'exec','exec_status','env','sleep','timestamp',
        // Time
        'time_millis','time_format','time_parse','time_add','time_diff',
        // Crypto
        'encrypt','decrypt',
        // Logging
        'log_debug','log_info','log_warn','log_error',
        // Math
        'abs','sqrt','floor','ceil','round','min','max','pow','parse_int','parse_float',
        // Assertions
        'assert','assert_eq','assert_ne','assert_true','assert_false','assert_contains','assert_throws',
        // Regex
        'regex_match','regex_find_all','regex_replace',
        // Collections / strings
        'len','push','pop','remove','contains','sort','filter','find','any','all',
        'keys','values','split','join','trim','to_upper','to_lower',
        'starts_with','ends_with','replace','substring','index_of',
        'pad_left','pad_right','char_at','chars','reverse','repeat'
    ],
    constants: ['true','false','null'],

    tokenizer: {
        root: [
            // Annotations  @[Foo]  @[Foo(bar)]
            [/@\[[a-zA-Z_]\w*(?:\([^)]*\))?\]/, 'annotation'],

            // Multiline strings  """..."""
            [/"""/, { token: 'string.triple', next: '@tripleString' }],

            // Interpolated strings  $"..."
            [/\$"/, { token: 'string.interp.delim', next: '@interpString' }],

            // Regular strings
            [/"/, { token: 'string.delim', next: '@string' }],

            // Line comment
            [/\/\/.*$/, 'comment'],

            // Block comment
            [/\/\*/, { token: 'comment', next: '@blockComment' }],

            // Numbers
            [/\b\d+\.\d+\b/, 'number.float'],
            [/\b\d+\b/, 'number'],

            // Identifiers → keyword dispatch
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

            // Brackets
            [/[{}()\[\]]/, '@brackets'],

            // Operators / punctuation
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

// ── VS Code dark+ theme for Monaco ────────────────────────────────────────────
const VARG_THEME = {
    base: 'vs-dark',
    inherit: true,
    rules: [
        { token: 'comment',           foreground: '6a9955', fontStyle: 'italic' },
        { token: 'keyword.control',   foreground: 'c586c0' },
        { token: 'keyword.other',     foreground: '569cd6' },
        { token: 'type',              foreground: '4ec9b0' },
        { token: 'support.function',  foreground: 'dcdcaa' },
        { token: 'constant.language', foreground: '569cd6' },
        { token: 'number',            foreground: 'b5cea8' },
        { token: 'number.float',      foreground: 'b5cea8' },
        { token: 'string',            foreground: 'ce9178' },
        { token: 'string.triple',     foreground: 'ce9178' },
        { token: 'string.interp',     foreground: 'ce9178' },
        { token: 'string.delim',      foreground: 'ce9178' },
        { token: 'string.interp.delim', foreground: 'ce9178' },
        { token: 'string.interp.expr',  foreground: '9cdcfe' },
        { token: 'string.escape',     foreground: 'd7ba7d' },
        { token: 'annotation',        foreground: 'dcdcaa', fontStyle: 'bold' },
        { token: 'identifier',        foreground: '9cdcfe' },
        { token: 'operator',          foreground: 'd4d4d4' },
        { token: 'delimiter',         foreground: 'd4d4d4' },
    ],
    colors: {
        'editor.background':              '#1e1e1e',
        'editor.foreground':              '#d4d4d4',
        'editorLineNumber.foreground':    '#858585',
        'editorActiveLineNumber.foreground': '#c6c6c6',
        'editor.lineHighlightBackground':'#2a2d2e',
        'editorCursor.foreground':        '#aeafad',
        'editor.selectionBackground':     '#264f78',
        'editorIndentGuide.background1':  '#404040',
        'editorIndentGuide.activeBackground1': '#707070',
    }
};

// ── Public API ────────────────────────────────────────────────────────────────
window.vargIde = {
    _editor: null,

    init: function (editorEl, resizeEl) {
        require.config({
            paths: { vs: 'https://cdn.jsdelivr.net/npm/monaco-editor@0.47.0/min/vs' }
        });

        require(['vs/editor/editor.main'], function () {
            // Register language
            monaco.languages.register({ id: 'varg', extensions: ['.varg'], aliases: ['Varg'] });
            monaco.languages.setMonarchTokensProvider('varg', MONARCH);

            // Register theme
            monaco.editor.defineTheme('varg-dark', VARG_THEME);

            // Create editor
            window.vargIde._editor = monaco.editor.create(editorEl, {
                value:                  DEFAULT_CODE,
                language:               'varg',
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
                dragStart    = e.clientY;
                panelStartH  = panel.offsetHeight;
                document.body.style.cursor = 'row-resize';
                document.body.style.userSelect = 'none';

                function onMove(e) {
                    const delta  = dragStart - e.clientY;
                    const newH   = Math.max(80, Math.min(window.innerHeight * 0.75, panelStartH + delta));
                    const rows   = shell.style.gridTemplateRows.split(' ');
                    rows[3]      = newH + 'px';
                    shell.style.gridTemplateRows = rows.join(' ');
                }
                function onUp() {
                    document.body.style.cursor = '';
                    document.body.style.userSelect = '';
                    window.removeEventListener('mousemove', onMove);
                    window.removeEventListener('mouseup',   onUp);
                    if (window.vargIde._editor) window.vargIde._editor.layout();
                }
                window.addEventListener('mousemove', onMove);
                window.addEventListener('mouseup',   onUp);
                e.preventDefault();
            });
        }
    },

    getCode: function () {
        return window.vargIde._editor ? window.vargIde._editor.getValue() : '';
    },

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
