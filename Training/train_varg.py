from unsloth import FastLanguageModel
from datasets import load_dataset
from trl import SFTTrainer
from transformers import TrainingArguments

# 1. Konfiguration
max_seq_length = 2048 
# Wir nutzen hier ein sehr starkes 27B/26B Modell, das durch 4-bit Quantisierung
# perfekt in deine 16GB RTX 5070 Ti passt.
model_name = "unsloth/gemma-2-27b-it-bnb-4bit" 

print("Lade Basis-Modell in die RTX 5070 Ti...")
model, tokenizer = FastLanguageModel.from_pretrained(
    model_name = model_name,
    max_seq_length = max_seq_length,
    load_in_4bit = True,
)

# 2. Das "Lernzentrum" (LoRA Adapter) aktivieren
print("Initialisiere Varg-Lernzentrum...")
model = FastLanguageModel.get_peft_model(
    model,
    r = 16, 
    target_modules = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"],
    lora_alpha = 16,
    lora_dropout = 0, 
    bias = "none",
    use_gradient_checkpointing = "unsloth",
)

# 3. Daten formatieren (Gemma's Chat-Format)
def formatting_prompts_func(examples):
    instructions = examples["instruction"]
    outputs      = examples["output"]
    texts = []
    for instruction, output in zip(instructions, outputs):
        text = f"<start_of_turn>user\n{instruction}<end_of_turn>\n<start_of_turn>model\n{output}<end_of_turn>"
        texts.append(text)
    return { "text" : texts, }

print("Lade deine varg_trainingsdaten.jsonl...")
dataset = load_dataset("json", data_files="varg_trainingsdaten.jsonl", split="train")
dataset = dataset.map(formatting_prompts_func, batched = True,)

# 4. Training konfigurieren
print("Starte das Training! Lehn dich zurück...")
trainer = SFTTrainer(
    model = model,
    tokenizer = tokenizer,
    train_dataset = dataset,
    dataset_text_field = "text",
    max_seq_length = max_seq_length,
    dataset_num_proc = 2,
    args = TrainingArguments(
        per_device_train_batch_size = 2,
        gradient_accumulation_steps = 4,
        warmup_steps = 10,
        num_train_epochs = 3, # Wir gehen deine 500+ Zeilen exakt 3x durch, damit das Modell Varg auswendig lernt
        learning_rate = 2e-4,
        fp16 = not FastLanguageModel.is_bfloat16_supported(),
        bf16 = FastLanguageModel.is_bfloat16_supported(),
        logging_steps = 1,
        optim = "adamw_8bit",
        output_dir = "varg_outputs",
    ),
)
trainer.train()

# 5. Export als Ollama-Datei (GGUF)
print("Training beendet! Exportiere GGUF-Datei für Ollama...")
model.save_pretrained_gguf("varg_model_gguf", tokenizer, quantization_method = "q4_k_m")
print("🔥 ALLES FERTIG! Dein Varg-Modell liegt im Ordner varg_model_gguf 🔥")